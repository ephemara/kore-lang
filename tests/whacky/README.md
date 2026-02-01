# KORE WHACKY TESTS - README
# What the hell are these files?

This folder contains **absolutely insane** KORE code examples that push the language to its theoretical limits. These are not meant to compile (yet) - they're aspirational demonstrations of what KORE syntax and semantics are designed to express.

## The Collection

### `meta_compiler.kr` - Self-Modifying Compiler Pipeline
A compiler that compiles itself. Features:
- Comptime grammar parsing and parser generation
- Effect-typed state machines (FSM effect)
- Hygienic macros that generate optimizer passes
- Multi-target code emission (LLVM, WASM, SPIR-V)
- Actors tracking compilation phases

**Insanity Level:** 🔥🔥🔥🔥🔥

---

### `reactive_game_engine.kr` - ECS Game Engine
A complete game engine in ~350 lines:
- Entity-Component-System architecture
- Physics with collision detection
- Frustum culled rendering
- Input handling with pattern matching
- Procedural scene generation
- Debug UI overlay with JSX

**Insanity Level:** 🔥🔥🔥🔥

---

### `neural_network.kr` - ML Framework From Scratch
PyTorch-style neural networks without PyTorch:
- Tensor operations with shape tracking
- Full autograd engine with backward passes
- Layers: Linear, ReLU, Sigmoid, Softmax
- Optimizers: SGD with momentum, Adam
- DataLoader with batching and shuffling
- Training loop with validation

**Insanity Level:** 🔥🔥🔥🔥🔥

---

### `distributed_cluster.kr` - Raft Consensus + KV Store
Distributed systems primitives:
- Complete Raft consensus implementation
- Leader election and log replication
- Consistent hashing for partitioning
- Quorum reads/writes
- Failure detection and failover
- Actor-based node communication

**Insanity Level:** 🔥🔥🔥🔥🔥

---

### `fullstack_framework.kr` - Web Framework
React + Express + GraphQL + ORM in one:
- React-style hooks (useState, useEffect, useMemo, etc.)
- JSX component system
- Express-like router with middleware
- GraphQL schema definition and resolvers
- Query builder ORM with relations
- WebSocket rooms and broadcasting
- Session/auth middleware

**Insanity Level:** 🔥🔥🔥🔥🔥

---

### `audio_synthesizer.kr` - Real-Time DSP
Complete audio synthesis:
- Band-limited oscillators (polyBLEP)
- Biquad and Moog ladder filters
- ADSR envelopes
- Effects: delay, reverb, distortion, chorus
- Modular patching system (Eurorack-style)
- MIDI note and CC handling
- Polyphonic voice allocation

**Insanity Level:** 🔥🔥🔥🔥🔥

---

## Why These Exist

These files serve several purposes:

1. **Language Design Validation** - If KORE's syntax can express these concepts elegantly, the language design is sound.

2. **Documentation** - They show what idiomatic KORE looks like for complex domains.

3. **Aspiration** - This is where KORE is headed. The self-hosted compiler will eventually run all of this.

4. **Flex** - Because sometimes you just need to show that a language can theoretically implement an entire game engine, neural network framework, and distributed database in a single file each.

## Key KORE Features Demonstrated

| Feature | Files |
|---------|-------|
| **Actors** | All of them |
| **Effects** | All of them |
| **JSX** | reactive_game_engine, fullstack_framework |
| **Pattern Matching** | All of them |
| **Comptime** | meta_compiler |
| **Macros** | meta_compiler |
| **Traits/Generics** | neural_network, audio_synthesizer |
| **Async/Await** | distributed_cluster, fullstack_framework |
| **Ownership** | Implicit in all |

---

*These are the kinds of programs KORE is designed to make possible. Welcome to the future.*
