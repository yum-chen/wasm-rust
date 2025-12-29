#!/usr/bin/env python3
"""
MIR Dump Comparison Script

Compares MIR dumps between LLVM and Cranelift backends to ensure
semantic equivalence and identify any discrepancies.
"""

import json
import sys
import argparse
import os
from pathlib import Path
from typing import Dict, List, Any, Tuple
import subprocess

def load_mir_dump(file_path: str) -> Dict[str, Any]:
    """Load and parse a MIR dump file."""
    try:
        with open(file_path, 'r') as f:
            content = f.read()
        
        # Basic parsing - in a real implementation, this would use rustc's MIR parser
        return {
            'file_path': file_path,
            'functions': extract_functions(content),
            'globals': extract_globals(content),
            'types': extract_types(content),
            'raw_content': content
        }
    except Exception as e:
        print(f"Error loading MIR dump {file_path}: {e}")
        return {}

def extract_functions(content: str) -> List[Dict[str, Any]]:
    """Extract function definitions from MIR content."""
    functions = []
    lines = content.split('\n')
    current_function = None
    current_block = None
    indent_level = 0
    
    for line in lines:
        stripped = line.strip()
        
        # Function definition
        if stripped.startswith('fn ') or stripped.startswith('const fn '):
            if current_function:
                functions.append(current_function)
            
            current_function = {
                'name': stripped.split('(')[0].split()[-1],
                'signature': extract_signature(stripped),
                'blocks': [],
                'attributes': extract_attributes(stripped)
            }
            continue
            
        if current_function is None:
            continue
            
        # Basic block
        if stripped.endswith(':') and not stripped.startswith('}'):
            if current_block:
                current_function['blocks'].append(current_block)
                
            current_block = {
                'name': stripped[:-1],
                'statements': [],
                'terminator': None
            }
            continue
            
        if current_block is None:
            continue
            
        # Terminator
        if any(stripped.startswith(term) for term in ['return', 'unwind', 'goto', 'switchInt', 'resume', 'terminate', 'assert']):
            current_block['terminator'] = stripped
            continue
            
        # Regular statement
        if stripped and not stripped.startswith('//') and not stripped.startswith('#'):
            current_block['statements'].append(stripped)
    
    # Add the last function and block
    if current_function:
        if current_block:
            current_function['blocks'].append(current_block)
        functions.append(current_function)
    
    return functions

def extract_signature(line: str) -> str:
    """Extract function signature from MIR line."""
    start = line.find('(')
    end = line.find(')', start)
    if start != -1 and end != -1:
        return line[start:end+1]
    return '()'

def extract_attributes(line: str) -> List[str]:
    """Extract function attributes from MIR line."""
    attributes = []
    if '#[' in line:
        attrs_start = line.find('#[') + 2
        attrs_end = line.find(']', attrs_start)
        if attrs_end != -1:
            attrs_str = line[attrs_start:attrs_end]
            attributes = [attr.strip() for attr in attrs_str.split(',')]
    return attributes

def extract_globals(content: str) -> List[Dict[str, Any]]:
    """Extract global variable definitions."""
    globals = []
    for line in content.split('\n'):
        stripped = line.strip()
        if stripped.startswith('static ') or stripped.startswith('const '):
            globals.append({
                'definition': stripped,
                'name': stripped.split('=')[0].split()[-1],
                'type': extract_type_from_definition(stripped)
            })
    return globals

def extract_types(content: str) -> List[Dict[str, Any]]:
    """Extract type definitions."""
    types = []
    for line in content.split('\n'):
        stripped = line.strip()
        if stripped.startswith('type ') and '=' in stripped:
            types.append({
                'definition': stripped,
                'name': stripped.split('=')[0].split()[-1],
                'alias': stripped.split('=')[1].strip()
            })
    return types

def extract_type_from_definition(definition: str) -> str:
    """Extract type from variable definition."""
    parts = definition.split(':')
    if len(parts) > 1:
        return parts[1].split('=')[0].strip()
    return 'unknown'

def compare_functions(llvm_funcs: List[Dict], cranelift_funcs: List[Dict]) -> Dict[str, Any]:
    """Compare functions between LLVM and Cranelift outputs."""
    llvm_by_name = {f['name']: f for f in llvm_funcs}
    cranelift_by_name = {f['name']: f for f in cranelift_funcs}
    
    all_function_names = set(llvm_by_name.keys()) | set(cranelift_by_name.keys())
    
    comparison = {
        'functions': {},
        'missing_in_llvm': [],
        'missing_in_cranelift': [],
        'total_functions': len(all_function_names)
    }
    
    for name in all_function_names:
        llvm_func = llvm_by_name.get(name)
        cranelift_func = cranelift_by_name.get(name)
        
        if llvm_func is None:
            comparison['missing_in_llvm'].append(name)
            continue
            
        if cranelift_func is None:
            comparison['missing_in_cranelift'].append(name)
            continue
            
        func_comparison = compare_single_function(llvm_func, cranelift_func)
        comparison['functions'][name] = func_comparison
    
    return comparison

def compare_single_function(llvm_func: Dict, cranelift_func: Dict) -> Dict[str, Any]:
    """Compare a single function between backends."""
    comparison = {
        'signature_match': llvm_func['signature'] == cranelift_func['signature'],
        'attribute_match': set(llvm_func['attributes']) == set(cranelift_func['attributes']),
        'block_count_match': len(llvm_func['blocks']) == len(cranelift_func['blocks']),
        'blocks': {},
        'llvm_block_count': len(llvm_func['blocks']),
        'cranelift_block_count': len(cranelift_func['blocks'])
    }
    
    # Compare basic blocks
    llvm_blocks = {b['name']: b for b in llvm_func['blocks']}
    cranelift_blocks = {b['name']: b for b in cranelift_func['blocks']}
    
    all_block_names = set(llvm_blocks.keys()) | set(cranelift_blocks.keys())
    
    for block_name in all_block_names:
        llvm_block = llvm_blocks.get(block_name)
        cranelift_block = cranelift_blocks.get(block_name)
        
        if llvm_block is None or cranelift_block is None:
            comparison['blocks'][block_name] = {
                'present_in_llvm': llvm_block is not None,
                'present_in_cranelift': cranelift_block is not None,
                'statement_similarity': 0.0
            }
            continue
            
        block_comparison = compare_basic_blocks(llvm_block, cranelift_block)
        comparison['blocks'][block_name] = block_comparison
    
    return comparison

def compare_basic_blocks(llvm_block: Dict, cranelift_block: Dict) -> Dict[str, Any]:
    """Compare basic blocks between backends."""
    comparison = {
        'statement_count_match': len(llvm_block['statements']) == len(cranelift_block['statements']),
        'terminator_match': llvm_block['terminator'] == cranelift_block['terminator'],
        'statement_similarity': calculate_statement_similarity(
            llvm_block['statements'], 
            cranelift_block['statements']
        ),
        'llvm_statement_count': len(llvm_block['statements']),
        'cranelift_statement_count': len(cranelift_block['statements'])
    }
    
    return comparison

def calculate_statement_similarity(llvm_stmts: List[str], cranelift_stmts: List[str]) -> float:
    """Calculate similarity between statement lists."""
    if not llvm_stmts and not cranelift_stmts:
        return 1.0
    
    if not llvm_stmts or not cranelift_stmts:
        return 0.0
    
    # Simple string-based similarity - in practice, this would be more sophisticated
    llvm_set = set(normalize_statement(stmt) for stmt in llvm_stmts)
    cranelift_set = set(normalize_statement(stmt) for stmt in cranelift_stmts)
    
    intersection = len(llvm_set & cranelift_set)
    union = len(llvm_set | cranelift_set)
    
    return intersection / union if union > 0 else 0.0

def normalize_statement(stmt: str) -> str:
    """Normalize a statement for comparison."""
    # Remove whitespace and normalize common variations
    normalized = stmt.strip().lower()
    
    # Remove register names (which might differ between backends)
    import re
    normalized = re.sub(r'_\d+', '_N', normalized)
    
    # Normalize common patterns
    normalized = normalized.replace('move ', '')
    normalized = normalized.replace('copy ', '')
    
    return normalized

def compare_globals(llvm_globals: List[Dict], cranelift_globals: List[Dict]) -> Dict[str, Any]:
    """Compare global variables between backends."""
    llvm_by_name = {g['name']: g for g in llvm_globals}
    cranelift_by_name = {g['name']: g for g in cranelift_globals}
    
    comparison = {
        'match_count': 0,
        'mismatch_count': 0,
        'missing_in_llvm': [],
        'missing_in_cranelift': [],
        'details': {}
    }
    
    all_names = set(llvm_by_name.keys()) | set(cranelift_by_name.keys())
    
    for name in all_names:
        llvm_global = llvm_by_name.get(name)
        cranelift_global = cranelift_by_name.get(name)
        
        if llvm_global is None:
            comparison['missing_in_llvm'].append(name)
            continue
            
        if cranelift_global is None:
            comparison['missing_in_cranelift'].append(name)
            continue
            
        type_match = llvm_global['type'] == cranelift_global['type']
        if type_match:
            comparison['match_count'] += 1
        else:
            comparison['mismatch_count'] += 1
            
        comparison['details'][name] = {
            'type_match': type_match,
            'llvm_type': llvm_global['type'],
            'cranelift_type': cranelift_global['type']
        }
    
    return comparison

def generate_comparison_report(comparison: Dict[str, Any]) -> str:
    """Generate a human-readable comparison report."""
    report = []
    report.append("# MIR Parity Comparison Report")
    report.append("")
    
    # Summary
    total_funcs = comparison['functions']['total_functions']
    missing_llvm = len(comparison['functions']['missing_in_llvm'])
    missing_cranelift = len(comparison['functions']['missing_in_cranelift'])
    
    report.append("## Summary")
    report.append(f"- Total functions: {total_funcs}")
    report.append(f"- Missing in LLVM backend: {missing_llvm}")
    report.append(f"- Missing in Cranelift backend: {missing_cranelift}")
    
    # Function details
    report.append("")
    report.append("## Function Details")
    
    for func_name, func_comparison in comparison['functions']['functions'].items():
        report.append(f"### {func_name}")
        
        sig_match = "✅" if func_comparison['signature_match'] else "❌"
        attr_match = "✅" if func_comparison['attribute_match'] else "❌"
        block_match = "✅" if func_comparison['block_count_match'] else "❌"
        
        report.append(f"- Signature match: {sig_match}")
        report.append(f"- Attributes match: {attr_match}")
        report.append(f"- Block count match: {block_match}")
        report.append(f"- LLVM blocks: {func_comparison['llvm_block_count']}")
        report.append(f"- Cranelift blocks: {func_comparison['cranelift_block_count']}")
        
        # Block details
        report.append("")
        for block_name, block_comparison in func_comparison['blocks'].items():
            similarity = block_comparison['statement_similarity']
            similarity_pct = f"{similarity*100:.1f}%"
            
            present_llvm = "✅" if block_comparison.get('present_in_llvm', True) else "❌"
            present_cranelift = "✅" if block_comparison.get('present_in_cranelift', True) else "❌"
            
            report.append(f"  - Block `{block_name}`: {similarity_pct} similarity")
            report.append(f"    Present in LLVM: {present_llvm}")
            report.append(f"    Present in Cranelift: {present_cranelift}")
        
        report.append("")
    
    return "\n".join(report)

def main():
    parser = argparse.ArgumentParser(description="Compare MIR dumps between LLVM and Cranelift backends")
    parser.add_argument("--llvm-dir", required=True, help="Directory containing LLVM MIR dumps")
    parser.add_argument("--cranelift-dir", required=True, help="Directory containing Cranelift MIR dumps")
    parser.add_argument("--output", required=True, help="Output JSON file for comparison results")
    
    args = parser.parse_args()
    
    llvm_dir = Path(args.llvm_dir)
    cranelift_dir = Path(args.cranelift_dir)
    output_file = Path(args.output)
    
    # Find all MIR files
    llvm_files = list(llvm_dir.glob("*.mir"))
    cranelift_files = list(cranelift_dir.glob("*.mir"))
    
    if not llvm_files:
        print(f"No MIR files found in {llvm_dir}")
        sys.exit(1)
        
    if not cranelift_files:
        print(f"No MIR files found in {cranelift_dir}")
        sys.exit(1)
    
    # Load and compare MIR dumps
    comparison_results = {
        'files_compared': [],
        'overall_similarity': 0.0,
        'function_comparison': {},
        'global_comparison': {},
        'issues_found': []
    }
    
    total_similarity = 0.0
    file_count = 0
    
    for llvm_file in llvm_files:
        cranelift_file = cranelift_dir / llvm_file.name
        
        if not cranelift_file.exists():
            comparison_results['issues_found'].append(f"Cranelift output missing for {llvm_file.name}")
            continue
        
        llvm_mir = load_mir_dump(str(llvm_file))
        cranelift_mir = load_mir_dump(str(cranelift_file))
        
        if not llvm_mir or not cranelift_mir:
            comparison_results['issues_found'].append(f"Failed to parse MIR for {llvm_file.name}")
            continue
        
        # Compare functions
        func_comparison = compare_functions(llvm_mir['functions'], cranelift_mir['functions'])
        
        # Compare globals
        global_comparison = compare_globals(llvm_mir['globals'], cranelift_mir['globals'])
        
        # Calculate overall similarity
        similarities = []
        for func_comp in func_comparison['functions'].values():
            for block_comp in func_comp['blocks'].values():
                similarities.append(block_comp['statement_similarity'])
        
        file_similarity = sum(similarities) / len(similarities) if similarities else 0.0
        total_similarity += file_similarity
        file_count += 1
        
        comparison_results['files_compared'].append({
            'file': llvm_file.name,
            'similarity': file_similarity,
            'function_comparison': func_comparison,
            'global_comparison': global_comparison
        })
    
    comparison_results['overall_similarity'] = total_similarity / file_count if file_count > 0 else 0.0
    
    # Write results
    output_file.parent.mkdir(parents=True, exist_ok=True)
    with open(output_file, 'w') as f:
        json.dump(comparison_results, f, indent=2)
    
    # Generate human-readable report
    report_file = output_file.with_suffix('.md')
    report = generate_comparison_report({
        'functions': comparison_results['overall_similarity']
    })
    
    with open(report_file, 'w') as f:
        f.write(report)
    
    print(f"MIR comparison completed. Overall similarity: {comparison_results['overall_similarity']:.2%}")
    print(f"Results saved to {output_file}")
    print(f"Report saved to {report_file}")

if __name__ == "__main__":
    main()
