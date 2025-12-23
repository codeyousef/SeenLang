# Seen Language: Infrastructure & Displacement Feature Spec

_Technical requirements for the UWW 4-Core Infrastructure and Firefox Oxidation._

## 1. Safety & Sandboxing (The "Capability" Layer)

_Objective: Enforce the "Neutral Network" and "Private Compute" invariants at the language level._

### A. Static Capability Tokens

- **Feature:** Function-level constraints that require a `token` to access specific syscalls or FFI boundaries (e.g., `fn mix(p: Packet) -> Result using NetToken`).
    
- **Requirement for UWW:** Used in the **Firefox Sidecar** to ensure that a Seen module can _only_ touch the Mixnet and is physically barred from accessing the local file system or legacy C++ networking.
    

### B. Generational Handle Masking

- **Feature:** Xoring handles with a region-specific `Secret` to prevent memory probing.
    
- **Requirement for Stealth Registry:** Used in **Core 1 (Registry)**. Even if an attacker can read the raw RAM of a node, they cannot resolve identity handles into view keys without the node-specific ephemeral secret.
    

## 2. Low-Level Hardware (The "Enclave" Layer)

_Objective: Direct control over Trusted Execution Environments and Bit-Level Protocols._

### A. Intrinsic TEE Attestation

- **Feature:** Language primitives for `enclave_call`, `seal_data`, and `unseal_data` that compile directly to Intel SGX or AMD SEV instructions.
    
- **Requirement for Orchestration Core:** Allows **Core 3** nodes to provide a "hardware proof" to the client that the SaaS code (e.g., an HRMS worker) is running in a private, encrypted memory space.
    

### B. Deterministic Bit-Fields

- **Feature:** First-class `bitfield` types with guaranteed memory layout (big-endian/little-endian) that do not vary by compiler target.
    
- **Requirement for Sphinx Mixnet:** Necessary for matching 5-hop packet headers exactly across different hardware architectures (x86 vs ARM) without serialization overhead.
    

## 3. Financial Integrity (The "Economy" Layer)

_Objective: Preventing floating-point drift in NUDGE calculations._

### A. Fixed-Point Numerics (`Qm.n`)

- **Feature:** A native `fixed` type with developer-defined precision (e.g., `fixed8.24` for 8 bits of integer and 24 bits of fractional).
    
- **Requirement for Economy Core:** Essential for **Core 2** and the **Stewardship Tax**. It ensures that 1-cent NUDGE tickets and daily tax escalations result in the exact same value on every node in the parachain, preventing "Consensus Divergence."
    

### B. Checked Arithmetic Invariants

- **Feature:** Global compiler switches to force `panic-on-overflow` for all financial types, regardless of optimization level.
    
- **Requirement for Nudge Protocol:** Ensures that malicious "Double-Spend" or "Infinite Mint" exploits are caught by the hardware's overflow flags before they can affect the ledger.
    

## 4. Persistent Storage (The "VSD" Layer)

_Objective: Managing the Virtual Shard Drive with 0ms latency._

### A. Pointer Pinning (Non-Relocatable Regions)

- **Feature:** Attributes to mark a `region` as "pinned," preventing the OS or runtime from moving the memory during a paging operation.
    
- **Requirement for VSD Mapper:** Required for **Core 3**. When a 64KB shard is paged into the browser from the decentralized network, Seen ensures the pointers to that data remain valid for the duration of the task.
    

## Feature Matrix Summary (Metal Layer)

|   |   |   |
|---|---|---|
|**Area**|**Feature**|**Why UWW Needs It**|
|**Security**|Capability Tokens|Sandboxing Seen from C++|
|**Identity**|Handle Masking|Stealth Metadata Protection|
|**Compute**|TEE Intrinsics|Private SaaS Attestation|
|**Finance**|Fixed-Point Math|Deterministic NUDGE Economy|
|**Transport**|Bit-Fields|Sphinx Packet Performance|
|**Storage**|Pointer Pinning|0-Copy VSD Shard Access|