#!/usr/bin/env python3
"""
MIR Parity Check Script

Validates that MIR parity between LLVM and Cranelift backends
meets the required similarity threshold.
"""

import json
import sys
import argparse
from typing import Dict, Any

def load_comparison_results(file_path: str) -> Dict[str, Any]:
    """Load MIR comparison results from JSON file."""
    try:
        with open(file_path, 'r') as f:
            return json.load(f)
    except Exception as e:
        print(f"Error loading comparison results: {e}")
        sys.exit(1)

def calculate_overall_parity(comparison: Dict[str, Any]) -> float:
    """Calculate overall parity score from comparison results."""
    total_similarity = 0.0
    file_count = 0
    
    for file_result in comparison.get('files_compared', []):
        similarity = file_result.get('similarity', 0.0)
        total_similarity += similarity
        file_count += 1
    
    return total_similarity / file_count if file_count > 0 else 0.0

def check_function_parity(function_comparison: Dict[str, Any]) -> Dict[str, Any]:
    """Check parity for individual functions."""
    issues = []
    warnings = []
    
    for func_name, func_details in function_comparison.get('functions', {}).items():
        # Check signature matching
        if not func_details.get('signature_match', False):
            issues.append(f"Function {func_name}: Signature mismatch between backends")
        
        # Check attribute matching
        if not func_details.get('attribute_match', False):
            warnings.append(f"Function {func_name}: Attribute differences detected")
        
        # Check basic block structure
        if not func_details.get('block_count_match', False):
            issues.append(f"Function {func_name}: Different number of basic blocks")
        
        # Check block-level similarity
        for block_name, block_details in func_details.get('blocks', {}).items():
            similarity = block_details.get('statement_similarity', 0.0)
            if similarity < 0.8:  # 80% similarity threshold for blocks
                warnings.append(
                    f"Function {func_name}, Block {block_name}: Low similarity ({similarity:.1%})"
                )
            
            if not block_details.get('present_in_llvm', True):
                issues.append(f"Function {func_name}, Block {block_name}: Missing from LLVM backend")
            
            if not block_details.get('present_in_cranelift', True):
                issues.append(f"Function {func_name}, Block {block_name}: Missing from Cranelift backend")
    
    return {
        'issues': issues,
        'warnings': warnings,
        'total_issues': len(issues),
        'total_warnings': len(warnings)
    }

def check_global_parity(global_comparison: Dict[str, Any]) -> Dict[str, Any]:
    """Check parity for global variables."""
    issues = []
    warnings = []
    
    missing_llvm = global_comparison.get('missing_in_llvm', [])
    missing_cranelift = global_comparison.get('missing_in_cranelift', [])
    
    for name in missing_llvm:
        issues.append(f"Global {name}: Missing from LLVM backend")
    
    for name in missing_cranelift:
        issues.append(f"Global {name}: Missing from Cranelift backend")
    
    # Check type mismatches
    details = global_comparison.get('details', {})
    for name, detail in details.items():
        if not detail.get('type_match', False):
            warnings.append(
                f"Global {name}: Type mismatch "
                f"(LLVM: {detail.get('llvm_type', 'unknown')}, "
                f"Cranelift: {detail.get('cranelift_type', 'unknown')})"
            )
    
    return {
        'issues': issues,
        'warnings': warnings,
        'total_issues': len(issues),
        'total_warnings': len(warnings)
    }

def generate_parity_report(comparison: Dict[str, Any], threshold: float) -> str:
    """Generate a detailed parity report."""
    report = []
    report.append("# MIR Parity Check Report")
    report.append("")
    
    overall_parity = calculate_overall_parity(comparison)
    report.append(f"## Overall Parity Score: {overall_parity:.2%}")
    report.append(f"## Required Threshold: {threshold:.2%}")
    
    if overall_parity >= threshold:
        report.append("✅ **PARITY REQUIREMENT MET**")
    else:
        report.append("❌ **PARITY REQUIREMENT FAILED**")
    
    report.append("")
    
    # File-by-file breakdown
    report.append("## File-by-File Analysis")
    
    for file_result in comparison.get('files_compared', []):
        file_name = file_result.get('file', 'unknown')
        similarity = file_result.get('similarity', 0.0)
        
        status = "✅" if similarity >= threshold else "❌"
        report.append(f"- `{file_name}`: {similarity:.1%} similarity {status}")
    
    report.append("")
    
    # Check function parity
    total_function_issues = 0
    total_function_warnings = 0
    
    for file_result in comparison.get('files_compared', []):
        func_comparison = file_result.get('function_comparison', {})
        function_parity = check_function_parity(func_comparison)
        
        total_function_issues += function_parity['total_issues']
        total_function_warnings += function_parity['total_warnings']
    
    # Check global parity
    total_global_issues = 0
    total_global_warnings = 0
    
    for file_result in comparison.get('files_compared', []):
        global_comparison = file_result.get('global_comparison', {})
        global_parity = check_global_parity(global_comparison)
        
        total_global_issues += global_parity['total_issues']
        total_global_warnings += global_parity['total_warnings']
    
    # Summary
    report.append("## Summary")
    report.append(f"- Total files compared: {len(comparison.get('files_compared', []))}")
    report.append(f"- Total function issues: {total_function_issues}")
    report.append(f"- Total function warnings: {total_function_warnings}")
    report.append(f"- Total global issues: {total_global_issues}")
    report.append(f"- Total global warnings: {total_global_warnings}")
    
    # Critical issues
    all_issues = []
    for file_result in comparison.get('files_compared', []):
        all_issues.extend(comparison.get('issues_found', []))
    
    if all_issues:
        report.append("")
        report.append("## Critical Issues")
        for issue in all_issues:
            report.append(f"- ❌ {issue}")
    
    # Recommendations
    report.append("")
    report.append("## Recommendations")
    
    if overall_parity < threshold:
        report.append("- **PARITY ISSUE**: Investigate differences in MIR generation")
        report.append("- Review missing functions or basic blocks")
        report.append("- Check for systematic translation errors")
    else:
        report.append("- Parity requirements met. Continue monitoring.")
    
    if total_function_warnings > 0:
        report.append("- Address function-level warnings for improved consistency")
    
    if total_global_warnings > 0:
        report.append("- Review global variable type differences")
    
    return "\n".join(report)

def main():
    parser = argparse.ArgumentParser(description="Check MIR parity between backends")
    parser.add_argument("--input", required=True, help="MIR comparison JSON file")
    parser.add_argument("--threshold", type=float, default=0.95, 
                       help="Minimum similarity threshold (default: 0.95)")
    
    args = parser.parse_args()
    
    # Load comparison results
    comparison = load_comparison_results(args.input)
    
    # Generate parity report
    report = generate_parity_report(comparison, args.threshold)
    
    # Print report
    print(report)
    
    # Determine exit code
    overall_parity = calculate_overall_parity(comparison)
    
    if overall_parity >= args.threshold:
        print("\n✅ MIR parity check PASSED")
        sys.exit(0)
    else:
        print("\n❌ MIR parity check FAILED")
        sys.exit(1)

if __name__ == "__main__":
    main()
