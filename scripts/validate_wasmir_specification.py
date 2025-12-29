#!/usr/bin/env python3
"""
WasmIR Specification Validation Tool

Validates that the WasmIR specification is complete and
covers all required instruction categories and type semantics.
"""

import json
import sys
import argparse
import re
from pathlib import Path
from typing import Dict, List, Set, Any, Tuple
from dataclasses import dataclass

@dataclass
class ValidationResult:
    name: str
    passed: bool
    issues: List[str]
    coverage: float
    
    def __str__(self):
        status = "✅ PASS" if self.passed else "❌ FAIL"
        return f"{self.name}: {status} ({self.coverage:.1%} coverage)"

@dataclass
class SpecificationValidation:
    instruction_coverage: ValidationResult
    type_system_coverage: ValidationResult
    memory_model_coverage: ValidationResult
    ownership_semantics: ValidationResult
    linear_type_support: ValidationResult
    reference_type_support: ValidationResult
    overall_coverage: float
    
    def summary(self) -> str:
        sections = [
            f"## Instruction Coverage: {self.instruction_coverage}",
            f"## Type System Coverage: {self.type_system_coverage}",
            f"## Memory Model Coverage: {self.memory_model_coverage}",
            f"## Ownership Semantics: {self.ownership_semantics}",
            f"## Linear Type Support: {self.linear_type_support}",
            f"## Reference Type Support: {self.reference_type_support}",
        ]
        
        summary = ["# WasmIR Specification Validation Report"]
        summary.extend(sections)
        summary.append(f"\n## Overall Coverage: {self.overall_coverage:.1%}")
        
        all_passed = all([
            self.instruction_coverage.passed,
            self.type_system_coverage.passed,
            self.memory_model_coverage.passed,
            self.ownership_semantics.passed,
            self.linear_type_support.passed,
            self.reference_type_support.passed,
        ])
        
        status = "✅ COMPLETE" if all_passed else "❌ INCOMPLETE"
        summary.append(f"\n## Final Status: {status}")
        
        return "\n".join(summary)

class WasmIRValidator:
    def __init__(self, specification_file: Path):
        self.spec_file = specification_file
        self.spec_content = self.load_specification()
        
    def load_specification(self) -> str:
        try:
            with open(self.spec_file, 'r') as f:
                return f.read()
        except Exception as e:
            raise RuntimeError(f"Failed to load specification: {e}")
    
    def validate_instruction_coverage(self) -> ValidationResult:
        """Validates that all required instruction categories are covered."""
        
        required_categories = {
            'Arithmetic Operations': [
                'Add', 'Sub', 'Mul', 'Div', 'Mod'
            ],
            'Bitwise Operations': [
                'And', 'Or', 'Xor', 'Shl', 'Shr', 'Not'
            ],
            'Comparison Operations': [
                'Eq', 'Ne', 'Lt', 'Le', 'Gt', 'Ge'
            ],
            'Memory Operations': [
                'MemoryLoad', 'MemoryStore', 'MemoryAlloc', 'MemoryFree'
            ],
            'Control Flow': [
                'Branch', 'Jump', 'Switch', 'Return', 'Call'
            ],
            'Reference Type Operations': [
                'ExternRefLoad', 'ExternRefStore', 'ExternRefNew', 'ExternRefCast',
                'ExternRefIsNull', 'ExternRefEq', 'FuncRefNew', 'FuncRefCall',
                'FuncRefIsNull', 'FuncRefEq', 'CallIndirect'
            ],
            'Linear Type Operations': [
                'LinearOp', 'Consume', 'Move', 'Drop'
            ],
            'Atomic Operations': [
                'AtomicOp', 'Add', 'Sub', 'And', 'Or', 'Xor', 'Exchange',
                'CompareExchange'
            ],
            'Capability Operations': [
                'CapabilityCheck'
            ],
            'JavaScript Interop': [
                'JSMethodCall'
            ],
            'Component Model Operations': [
                'ComponentCall', 'ComponentImport', 'ComponentExport'
            ]
        }
        
        issues = []
        found_instructions = self.extract_instructions_from_spec()
        total_required = sum(len(instructions) for instructions in required_categories.values())
        found_count = 0
        
        for category, required_instructions in required_categories.items():
            category_issues = []
            category_found = 0
            
            for instruction in required_instructions:
                if any(instruction.lower() in found_inst.lower() for found_inst in found_instructions):
                    category_found += 1
                else:
                    category_issues.append(f"Missing instruction: {instruction}")
            
            found_count += category_found
            
            if category_issues:
                issues.append(f"{category}: {', '.join(category_issues)}")
        
        coverage = found_count / total_required if total_required > 0 else 0.0
        
        return ValidationResult(
            name="Instruction Coverage",
            passed=len(issues) == 0,
            issues=issues,
            coverage=coverage
        )
    
    def extract_instructions_from_spec(self) -> List[str]:
        """Extract all instruction names from specification."""
        instructions = set()
        
        # Look for instruction definitions in various formats
        patterns = [
            r'Instruction::([A-Za-z][A-Za-z0-9_]*)',  // Rust enum
            r'\*\* `([A-Za-z][A-Za-z0-9_]*)`',      // Markdown code
            r'`([A-Za-z][A-Za-z0-9_]*)`:',          // Definition format
            r'([A-Za-z][A-Za-z0-9_]*)\s*:',      // Simple format
        ]
        
        for pattern in patterns:
            matches = re.findall(pattern, self.spec_content)
            for match in matches:
                instructions.add(match)
        
        # Look for common instruction keywords
        instruction_keywords = [
            'load', 'store', 'add', 'sub', 'mul', 'div', 'mod',
            'and', 'or', 'xor', 'shl', 'shr', 'not',
            'eq', 'ne', 'lt', 'le', 'gt', 'ge',
            'branch', 'jump', 'switch', 'return', 'call',
            'externref', 'funcref', 'linear', 'consume',
            'atomic', 'capability', 'component', 'import', 'export'
        ]
        
        for line in self.spec_content.split('\n'):
            line_lower = line.lower()
            for keyword in instruction_keywords:
                if keyword in line_lower and keyword not in ['the', 'and', 'or', 'not']:
                    # Try to extract the full instruction name
                    words = re.findall(r'\b([a-z][a-z0-9_]*)\b', line)
                    for word in words:
                        if keyword in word and len(word) > 3:  # Avoid short matches
                            instructions.add(word)
        
        return list(instructions)
    
    def validate_type_system_coverage(self) -> ValidationResult:
        """Validates type system completeness."""
        
        required_types = {
            'Primitive Types': [
                'I32', 'I64', 'F32', 'F64', 'Bool', 'Void'
            ],
            'Reference Types': [
                'ExternRef', 'FuncRef', 'Ref', 'Pointer'
            ],
            'Linear Types': [
                'Linear', 'LinearType'
            ],
            'Capability Types': [
                'Capability', 'CapabilityType'
            ],
            'Composite Types': [
                'Array', 'Struct', 'Tuple', 'Union'
            ],
            'Parameterized Types': [
                'Generic', 'TypeParameter'
            ]
        }
        
        issues = []
        found_types = self.extract_types_from_spec()
        total_required = sum(len(types) for types in required_types.values())
        found_count = 0
        
        for category, required_type_list in required_types.items():
            category_issues = []
            category_found = 0
            
            for type_name in required_type_list:
                if any(type_name.lower() in found_type.lower() for found_type in found_types):
                    category_found += 1
                else:
                    category_issues.append(f"Missing type: {type_name}")
            
            found_count += category_found
            
            if category_issues:
                issues.append(f"{category}: {', '.join(category_issues)}")
        
        coverage = found_count / total_required if total_required > 0 else 0.0
        
        return ValidationResult(
            name="Type System Coverage",
            passed=len(issues) == 0,
            issues=issues,
            coverage=coverage
        )
    
    def extract_types_from_spec(self) -> List[str]:
        """Extract all type names from specification."""
        types = set()
        
        # Look for type definitions
        patterns = [
            r'type\s+([A-Z][A-Za-z0-9_]*)\s*=',     // type declaration
            r'struct\s+([A-Z][A-Za-z0-9_]*)\s*',    // struct declaration
            r'enum\s+([A-Z][A-Za-z0-9_]*)\s*',      // enum declaration
            r'Type::([A-Z][A-Za-z0-9_]*)',          // Rust enum variant
            r'`([A-Z][A-Za-z0-9_]*)`',               // Markdown code
        ]
        
        for pattern in patterns:
            matches = re.findall(pattern, self.spec_content, re.IGNORECASE)
            for match in matches:
                if len(match) > 2:  # Filter out very short matches
                    types.add(match)
        
        return list(types)
    
    def validate_memory_model_coverage(self) -> ValidationResult:
        """Validates memory model completeness."""
        
        required_concepts = [
            'Linear Memory',
            'Shared Memory', 
            'Memory Regions',
            'Memory Ordering',
            'Atomic Operations',
            'Memory Safety',
            'Garbage Collection',
            'Component Model Memory',
            'Memory Layout'
        ]
        
        issues = []
        found_concepts = self.extract_memory_concepts()
        found_count = 0
        
        for concept in required_concepts:
            if any(concept.lower() in found_concept.lower() for found_concept in found_concepts):
                found_count += 1
            else:
                issues.append(f"Missing concept: {concept}")
        
        coverage = found_count / len(required_concepts) if required_concepts else 0.0
        
        return ValidationResult(
            name="Memory Model Coverage",
            passed=len(issues) == 0,
            issues=issues,
            coverage=coverage
        )
    
    def extract_memory_concepts(self) -> List[str]:
        """Extract memory-related concepts from specification."""
        concepts = set()
        
        memory_keywords = [
            'memory', 'linear', 'shared', 'atomic', 'ordering',
            'region', 'garbage', 'collection', 'layout',
            'safety', 'component', 'model', 'heap', 'stack'
        ]
        
        for line in self.spec_content.split('\n'):
            line_lower = line.lower()
            for keyword in memory_keywords:
                if keyword in line_lower:
                    # Extract the context around the keyword
                    words = re.findall(r'\b([a-z][a-z\s]*memory)\b', line_lower)
                    for word in words:
                        if len(word.strip()) > 5:
                            concepts.add(word.strip())
        
        return list(concepts)
    
    def validate_ownership_semantics(self) -> ValidationResult:
        """Validates ownership semantics coverage."""
        
        required_concepts = [
            'Ownership Model',
            'Move Semantics',
            'Borrow Checking',
            'Lifetime Tracking',
            'Drop Semantics',
            'Linear Types',
            'Resource Management',
            'Ownership Transfer',
            'Borrowing Rules',
            'Lifetime Annotations'
        ]
        
        issues = []
        found_concepts = self.extract_ownership_concepts()
        found_count = 0
        
        for concept in required_concepts:
            if any(concept.lower() in found_concept.lower() for found_concept in found_concepts):
                found_count += 1
            else:
                issues.append(f"Missing ownership concept: {concept}")
        
        coverage = found_count / len(required_concepts) if required_concepts else 0.0
        
        return ValidationResult(
            name="Ownership Semantics",
            passed=len(issues) == 0,
            issues=issues,
            coverage=coverage
        )
    
    def extract_ownership_concepts(self) -> List[str]:
        """Extract ownership-related concepts from specification."""
        concepts = set()
        
        ownership_keywords = [
            'ownership', 'move', 'borrow', 'lifetime', 'drop',
            'linear', 'resource', 'transfer', 'checking',
            'rules', 'annotations', 'consume'
        ]
        
        for line in self.spec_content.split('\n'):
            line_lower = line.lower()
            for keyword in ownership_keywords:
                if keyword in line_lower:
                    # Extract the full concept
                    words = re.findall(r'\b([a-z][a-z\s]*(?:ownership|move|borrow|lifetime|drop|linear|resource|transfer|checking|rules|annotations|consume))\b', 
                                     line_lower)
                    for word in words:
                        if len(word.strip()) > 4:
                            concepts.add(word.strip())
        
        return list(concepts)
    
    def validate_linear_type_support(self) -> ValidationResult:
        """Validates linear type implementation support."""
        
        required_features = [
            'Linear Type Definition',
            'Use-Once Semantics',
            'Consumption Operations',
            'Move Operations',
            'Drop Enforcement',
            'Linear Function Calls',
            'Linear Struct Definitions',
            'Linear Type Parameters',
            'Linear Type Checking'
        ]
        
        issues = []
        found_features = self.extract_linear_features()
        found_count = 0
        
        for feature in required_features:
            if any(feature.lower() in found_feature.lower() for found_feature in found_features):
                found_count += 1
            else:
                issues.append(f"Missing linear feature: {feature}")
        
        coverage = found_count / len(required_features) if required_features else 0.0
        
        return ValidationResult(
            name="Linear Type Support",
            passed=len(issues) == 0,
            issues=issues,
            coverage=coverage
        )
    
    def extract_linear_features(self) -> List[str]:
        """Extract linear type features from specification."""
        features = set()
        
        linear_keywords = [
            'linear', 'consume', 'move', 'drop', 'use-once',
            'consumption', 'enforcement', 'checking', 'parameter'
        ]
        
        for line in self.spec_content.split('\n'):
            if 'linear' in line.lower():
                words = re.findall(r'\b([a-z][a-z\s]*linear[a-z\s]*[a-z]*)\b', line.lower())
                for word in words:
                    if len(word.strip()) > 6:
                        features.add(word.strip())
        
        return list(features)
    
    def validate_reference_type_support(self) -> ValidationResult:
        """Validates reference type implementation support."""
        
        required_operations = {
            'ExternRef': [
                'Creation', 'Deletion', 'Access', 'Method Call', 
                'Property Access', 'Null Check', 'Equality Check'
            ],
            'FuncRef': [
                'Creation', 'Deletion', 'Call', 'Indirect Call',
                'Null Check', 'Equality Check', 'Function Table Access'
            ],
            'SharedSlice': [
                'Creation', 'Access', 'Bounds Check', 'Iteration',
                'Atomic Operations', 'Thread Safety'
            ]
        }
        
        issues = []
        found_operations = self.extract_reference_operations()
        total_required = sum(len(operations) for operations in required_operations.values())
        found_count = 0
        
        for ref_type, required_ops in required_operations.items():
            type_issues = []
            type_found = 0
            
            for operation in required_ops:
                if any(operation.lower() in found_op.lower() for found_op in found_operations):
                    type_found += 1
                else:
                    type_issues.append(f"Missing operation: {operation}")
            
            found_count += type_found
            
            if type_issues:
                issues.append(f"{ref_type}: {', '.join(type_issues)}")
        
        coverage = found_count / total_required if total_required > 0 else 0.0
        
        return ValidationResult(
            name="Reference Type Support",
            passed=len(issues) == 0,
            issues=issues,
            coverage=coverage
        )
    
    def extract_reference_operations(self) -> List[str]:
        """Extract reference type operations from specification."""
        operations = set()
        
        ref_keywords = [
            'externref', 'funcref', 'sharedslice',
            'create', 'delete', 'access', 'call', 'method',
            'property', 'null', 'equal', 'table', 'bounds',
            'iteration', 'atomic', 'thread', 'safety'
        ]
        
        for line in self.spec_content.split('\n'):
            line_lower = line.lower()
            for keyword in ref_keywords:
                if keyword in line_lower:
                    # Extract operations related to this keyword
                    words = re.findall(r'\b([a-z][a-z]*[a-z]+(?:\s+[a-z]+)?)\b', line_lower)
                    for word in words:
                        if any(op in word for op in ['create', 'delete', 'access', 'call', 'method', 'property']):
                            if len(word.strip()) > 3:
                                operations.add(word.strip())
        
        return list(operations)
    
    def run_validation(self) -> SpecificationValidation:
        """Run complete specification validation."""
        
        print("Validating WasmIR specification...")
        
        instruction_validation = self.validate_instruction_coverage()
        print(f"Instruction validation: {instruction_validation}")
        
        type_validation = self.validate_type_system_coverage()
        print(f"Type system validation: {type_validation}")
        
        memory_validation = self.validate_memory_model_coverage()
        print(f"Memory model validation: {memory_validation}")
        
        ownership_validation = self.validate_ownership_semantics()
        print(f"Ownership semantics validation: {ownership_validation}")
        
        linear_validation = self.validate_linear_type_support()
        print(f"Linear type validation: {linear_validation}")
        
        reference_validation = self.validate_reference_type_support()
        print(f"Reference type validation: {reference_validation}")
        
        overall_coverage = (
            instruction_validation.coverage +
            type_validation.coverage +
            memory_validation.coverage +
            ownership_validation.coverage +
            linear_validation.coverage +
            reference_validation.coverage
        ) / 6.0
        
        return SpecificationValidation(
            instruction_coverage=instruction_validation,
            type_system_coverage=type_validation,
            memory_model_coverage=memory_validation,
            ownership_semantics=ownership_validation,
            linear_type_support=linear_validation,
            reference_type_support=reference_validation,
            overall_coverage=overall_coverage
        )

def main():
    parser = argparse.ArgumentParser(description="Validate WasmIR specification completeness")
    parser.add_argument("specification", help="Path to WasmIR specification file")
    parser.add_argument("--output", "-o", help="Output validation report to file")
    parser.add_argument("--json", action="store_true", help="Output results in JSON format")
    
    args = parser.parse_args()
    
    spec_file = Path(args.specification)
    if not spec_file.exists():
        print(f"Error: Specification file {spec_file} does not exist")
        sys.exit(1)
    
    validator = WasmIRValidator(spec_file)
    validation = validator.run_validation()
    
    report = validation.summary()
    
    if args.output:
        with open(args.output, 'w') as f:
            f.write(report)
        print(f"Validation report written to {args.output}")
    else:
        print("\n" + report)
    
    # Exit with appropriate code
    all_passed = all([
        validation.instruction_coverage.passed,
        validation.type_system_coverage.passed,
        validation.memory_model_coverage.passed,
        validation.ownership_semantics.passed,
        validation.linear_type_support.passed,
        validation.reference_type_support.passed,
    ])
    
    if args.json:
        json_data = {
            'instruction_coverage': {
                'passed': validation.instruction_coverage.passed,
                'coverage': validation.instruction_coverage.coverage,
                'issues': validation.instruction_coverage.issues
            },
            'type_system_coverage': {
                'passed': validation.type_system_coverage.passed,
                'coverage': validation.type_system_coverage.coverage,
                'issues': validation.type_system_coverage.issues
            },
            'memory_model_coverage': {
                'passed': validation.memory_model_coverage.passed,
                'coverage': validation.memory_model_coverage.coverage,
                'issues': validation.memory_model_coverage.issues
            },
            'ownership_semantics': {
                'passed': validation.ownership_semantics.passed,
                'coverage': validation.ownership_semantics.coverage,
                'issues': validation.ownership_semantics.issues
            },
            'linear_type_support': {
                'passed': validation.linear_type_support.passed,
                'coverage': validation.linear_type_support.coverage,
                'issues': validation.linear_type_support.issues
            },
            'reference_type_support': {
                'passed': validation.reference_type_support.passed,
                'coverage': validation.reference_type_support.coverage,
                'issues': validation.reference_type_support.issues
            },
            'overall_coverage': validation.overall_coverage
        }
        
        output_file = args.output.replace('.md', '.json') if args.output else 'validation.json'
        with open(output_file, 'w') as f:
            json.dump(json_data, f, indent=2)
    
    sys.exit(0 if all_passed else 1)

if __name__ == "__main__":
    main()
