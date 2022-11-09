// Design:
//
// Instead of trying to interpret vr4300 instructions directly, the plan is to "compile" them into
// a bytecode first.
// Anything static, like instruction decoding or scheduling timing, will be baked into the bytecode
// so the operations should only be
//
// Each bytecode instruction will actually a be function pointer (or offset) plus some args.
// this is inspired by the cached interpreter in dolphin (though that doesn't have args).
//
// The goal is to take advantage of the advanced branch predictors found modern cpus
// (such as Intel's haswell or later, and AMD zen or later) that are actually very good
// at predicting sequences of indirect branches.
//
// And then taking inspiration from recent faster-python efforts, hot bytecode sequences can
// potentially be optimised into even faster bytecode.
//
// The hope is that this design can be fast enough that we don't need a JIT.
// But if not, this bytecode just so happens to be a good IL to JIT from.

