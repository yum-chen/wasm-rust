//! Linear Types MIR Passes
//! 
//! This module implements the required MIR passes for enforcing linear type semantics
//! in WasmRust compiler, as specified in the design document.

use rustc_middle::mir::*;
use rustc_middle::ty::{Ty, TyKind};
use rustc_middle::mir::visit::{MirVisitable, Visitor};
use rustc_data_structures::fx::FxHashMap;
use std::collections::HashSet;

/// Linear object destruction pass - prevents implicit drops of linear types
pub struct LinearObjectDropScan<'tcx> {
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    live_linear_vars: FxHashMap<Local, LinearVarState>,
    errors: Vec<LinearTypeError>,
}

#[derive(Debug, Clone, Copy)]
enum LinearVarState {
    Uninitialized,
    Active,
    Consumed,
}

#[derive(Debug)]
enum LinearTypeError {
    ImplicitDrop { var: Local, span: Span },
    UnconsumedReturn { var: Local, span: Span },
    UseAfterConsume { var: Local, span: Span },
    DoubleConsume { var: Local, span: Span },
}

impl<'tcx> LinearObjectDropScan<'tcx> {
    pub fn new(tcx: rustc_middle::ty::TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            live_linear_vars: FxHashMap::default(),
            errors: Vec::new(),
        }
    }

    pub fn analyze(&mut self, body: &Body<'tcx>) -> Vec<LinearTypeError> {
        // Discover linear variables
        self.discover_linear_vars(body);
        
        // Analyze each basic block
        for (bb_idx, bb_data) in body.basic_blocks.iter_enumerated() {
            self.analyze_basic_block(bb_idx, bb_data);
        }
        
        // Check return paths
        self.check_return_paths(body);
        
        std::mem::take(&mut self.errors)
    }

    fn discover_linear_vars(&mut self, body: &Body<'tcx>) {
        for (local, local_decl) in body.local_decls.iter_enumerated() {
            if self.is_linear_type(local_decl.ty) {
                self.live_linear_vars.insert(local, LinearVarState::Uninitialized);
            }
        }
    }

    fn is_linear_type(&self, ty: Ty<'tcx>) -> bool {
        // Check if type has #[wasm::linear] attribute
        // This would be detected via proc macro expansion or custom attribute
        match ty.kind() {
            TyKind::Adt(adt, _) => {
                // Check if ADT has linear attribute
                self.tcx.has_attr(adt.did(), sym::wasm_linear)
            }
            _ => false,
        }
    }

    fn analyze_basic_block(&mut self, bb_idx: BasicBlock, bb_data: &BasicBlockData<'tcx>) {
        for statement in &bb_data.statements {
            match statement.kind {
                StatementKind::StorageLive(local) => {
                    if let Some(state) = self.live_linear_vars.get_mut(&local) {
                        *state = LinearVarState::Active;
                    }
                }
                StatementKind::StorageDead(local) => {
                    if let Some(state) = self.live_linear_vars.get(&local) {
                        match state {
                            LinearVarState::Active => {
                                self.errors.push(LinearTypeError::ImplicitDrop {
                                    var: local,
                                    span: statement.source_info.span,
                                });
                            }
                            LinearVarState::Consumed => {
                                // Normal: consumed variable going out of scope
                                *state = LinearVarState::Uninitialized;
                            }
                            _ => {}
                        }
                    }
                }
                StatementKind::Assign(box (place, rvalue)) => {
                    self.handle_assignment(place, rvalue, statement.source_info.span);
                }
                _ => {}
            }
        }

        // Handle terminator
        self.analyze_terminator(bb_idx, &bb_data.terminator);
    }

    fn handle_assignment(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, span: Span) {
        // Check if we're moving a linear variable
        if let Some((local, _proj)) = place.as_local() {
            if self.is_linear_type(place.ty(self.tcx, body)) {
                // This is a move into a linear variable
                self.consume_linear_var(*local, span);
            }
        }

        // Analyze rvalue for linear variable consumption
        self.analyze_rvalue(rvalue, span);
    }

    fn analyze_rvalue(&mut self, rvalue: &Rvalue<'tcx>, span: Span) {
        match rvalue {
            Rvalue::Use(operand) | Rvalue::Repeat(operand, _) => {
                self.analyze_operand(operand, span);
            }
            Rvalue::BinaryOp(_, lhs, rhs) | Rvalue::CheckedBinaryOp(_, lhs, rhs) => {
                self.analyze_operand(lhs, span);
                self.analyze_operand(rhs, span);
            }
            Rvalue::UnaryOp(_, operand) => {
                self.analyze_operand(operand, span);
            }
            Rvalue::Aggregate(_, operands) => {
                for operand in operands {
                    self.analyze_operand(operand, span);
                }
            }
            _ => {}
        }
    }

    fn analyze_operand(&mut self, operand: &Operand<'tcx>, span: Span) {
        match operand {
            Operand::Copy(place) | Operand::Move(place) => {
                if let Some((local, _proj)) = place.as_local() {
                    if let Some(state) = self.live_linear_vars.get(&local) {
                        match state {
                            LinearVarState::Active => {
                                if matches!(operand, Operand::Move(_)) {
                                    self.consume_linear_var(*local, span);
                                }
                                // Copy is okay for non-consuming use
                            }
                            LinearVarState::Consumed => {
                                self.errors.push(LinearTypeError::UseAfterConsume {
                                    var: *local,
                                    span,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn consume_linear_var(&mut self, local: Local, span: Span) {
        if let Some(state) = self.live_linear_vars.get_mut(&local) {
            match state {
                LinearVarState::Active => {
                    *state = LinearVarState::Consumed;
                }
                LinearVarState::Uninitialized => {
                    // This might be a move from uninitialized variable
                    // Let the borrow checker handle this case
                }
                LinearVarState::Consumed => {
                    self.errors.push(LinearTypeError::DoubleConsume {
                        var: local,
                        span,
                    });
                }
            }
        }
    }

    fn analyze_terminator(&mut self, bb_idx: BasicBlock, terminator: &Terminator<'tcx>) {
        match terminator.kind {
            TerminatorKind::Call {
                func,
                args,
                destination,
                ..
            } => {
                // Analyze function arguments for linear consumption
                for operand in args {
                    self.analyze_operand(operand, terminator.source_info.span);
                }

                // If this is a consuming method call, mark the receiver as consumed
                if let Some((func_local, _)) = func.as_local() {
                    // This would need more sophisticated analysis to determine if it's a consuming call
                }
            }
            TerminatorKind::Return { .. } => {
                // Check return paths for unconsumed linear variables
            }
            _ => {}
        }
    }

    fn check_return_paths(&mut self, body: &Body<'tcx>) {
        for (local, state) in &self.live_linear_vars {
            match state {
                LinearVarState::Active => {
                    self.errors.push(LinearTypeError::UnconsumedReturn {
                        var: *local,
                        span: body.span,
                    });
                }
                _ => {}
            }
        }
    }
}

/// Linear path completeness analysis - ensures all paths consume linear variables
pub struct LinearPathCompleteness<'tcx> {
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    linear_vars: HashSet<Local>,
    path_states: Vec<FxHashMap<Local, LinearVarState>>,
    errors: Vec<LinearTypeError>,
}

impl<'tcx> LinearPathCompleteness<'tcx> {
    pub fn new(tcx: rustc_middle::ty::TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            linear_vars: HashSet::default(),
            path_states: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn analyze(&mut self, body: &Body<'tcx>) -> Vec<LinearTypeError> {
        // Initialize linear variable tracking
        self.discover_linear_vars(body);
        
        // Perform CFG analysis
        self.analyze_cfg(body);
        
        std::mem::take(&mut self.errors)
    }

    fn discover_linear_vars(&mut self, body: &Body<'tcx>) {
        for (local, local_decl) in body.local_decls.iter_enumerated() {
            if self.is_linear_type(local_decl.ty) {
                self.linear_vars.insert(local);
            }
        }
    }

    fn is_linear_type(&self, ty: Ty<'tcx>) -> bool {
        match ty.kind() {
            TyKind::Adt(adt, _) => {
                self.tcx.has_attr(adt.did(), sym::wasm_linear)
            }
            _ => false,
        }
    }

    fn analyze_cfg(&mut self, body: &Body<'tcx>) {
        // This would implement a full CFG traversal
        // For now, placeholder implementation
        
        // Start from entry block
        let entry_state = FxHashMap::default();
        self.analyze_cfg_recursive(body, START_BLOCK, entry_state);
    }

    fn analyze_cfg_recursive(
        &mut self,
        body: &Body<'tcx>,
        bb: BasicBlock,
        entry_state: FxHashMap<Local, LinearVarState>,
    ) {
        // Clone entry state for this path
        let mut current_state = entry_state.clone();
        
        // Analyze this block
        let bb_data = &body.basic_blocks[bb];
        self.analyze_block_with_state(bb_data, &mut current_state);
        
        // Recursively analyze successors
        for successor in bb_data.terminator.successors() {
            self.analyze_cfg_recursive(body, successor, current_state.clone());
        }
    }

    fn analyze_block_with_state(
        &mut self,
        bb_data: &BasicBlockData<'tcx>,
        state: &mut FxHashMap<Local, LinearVarState>,
    ) {
        for statement in &bb_data.statements {
            match statement.kind {
                StatementKind::StorageLive(local) => {
                    if self.linear_vars.contains(&local) {
                        state.insert(local, LinearVarState::Active);
                    }
                }
                StatementKind::StorageDead(local) => {
                    if self.linear_vars.contains(&local) {
                        state.remove(&local);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Linear capability escape check - prevents reference leaks
pub struct LinearCapabilityEscapeCheck<'tcx> {
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    linear_refs: FxHashMap<Local, RefState>,
    errors: Vec<LinearTypeError>,
}

#[derive(Debug, Clone)]
struct RefState {
    active_refs: Vec<Local>,
    consumption_point: Option<BasicBlock>,
}

#[derive(Debug)]
enum LinearRefError {
    LeakThroughBorrow { var: Local, span: Span },
    LeakThroughClosure { var: Local, span: Span },
}

impl<'tcx> LinearCapabilityEscapeCheck<'tcx> {
    pub fn new(tcx: rustc_middle::ty::TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            linear_refs: FxHashMap::default(),
            errors: Vec::new(),
        }
    }

    pub fn analyze(&mut self, body: &Body<'tcx>) -> Vec<LinearTypeError> {
        // This would implement borrow checking for linear types
        // For now, placeholder implementation
        
        std::mem::take(&mut self.errors)
    }
}

/// Panic path invariant - handles unwinding for linear types
pub struct PanicPathInvariant<'tcx> {
    tcx: rustc_middle::ty::TyCtxt<'tcx>,
    cleanup_blocks: HashSet<BasicBlock>,
    errors: Vec<LinearTypeError>,
}

impl<'tcx> PanicPathInvariant<'tcx> {
    pub fn new(tcx: rustc_middle::ty::TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            cleanup_blocks: HashSet::default(),
            errors: Vec::new(),
        }
    }

    pub fn analyze(&mut self, body: &Body<'tcx>) -> Vec<LinearTypeError> {
        // Identify cleanup blocks (unwind targets)
        self.discover_cleanup_blocks(body);
        
        // Verify cleanup block handling
        self.verify_cleanup_handling(body);
        
        std::mem::take(&mut self.errors)
    }

    fn discover_cleanup_blocks(&mut self, body: &Body<'tcx>) {
        for bb_data in &body.basic_blocks {
            if let TerminatorKind::Resume = bb_data.terminator.kind {
                // This is a cleanup block
                self.cleanup_blocks.insert(bb_data.terminator.successors().next().unwrap());
            }
        }
    }

    fn verify_cleanup_handling(&mut self, body: &Body<'tcx>) {
        // Verify that linear variables are properly handled in cleanup blocks
        for cleanup_bb in &self.cleanup_blocks {
            let bb_data = &body.basic_blocks[*cleanup_bb];
            
            // Check if cleanup block handles linear variables correctly
            for statement in &bb_data.statements {
                // This would need sophisticated analysis of cleanup logic
            }
        }
    }
}
