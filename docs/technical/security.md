# Security & Cryptography Specification

This document details the cryptographic algorithms, handshake verification, key derivation functions, and replay protection rules implemented across Owlmic.

Official test vectors verifying this specification are located in [`shared/protocol/vectors/crypto.json`](../../shared/protocol/vectors/crypto.json).

---

## 1. Cryptographic Primitives

Owlmic relies on established, standard NIST and IETF primitives:

| Purpose | Primitive | Standard | Parameters |
| :--- | :--- | :--- | :--- |
| **Identity & Asymmetric Key Exchange** | ECDH | NIST P-256 (secp256r1) | 65-byte uncompressed public keys (`0x04 \|\| x \|\| y`) |
| **Key Derivation Function** | HKDF | RFC 5869 | HKDF-SHA256, 32-byte outputs |
| **Transcript Hashing** | SHA-256 | FIPS 180-4 | 32-byte digest |
| **Authentication & MACs** | HMAC-SHA256 | RFC 2104 | 32-byte MAC |
| **Symmetric Authenticated Encryption** | AES-256-GCM | NIST SP 800-38D | 256-bit key, 96-bit nonce, 128-bit authentication tag |

---

## 2. Key Derivation & Handshake Authentication

```
                    Phone                                           PC
                      │                                             │
                      │─────── HELLO (Phone Static + Ephemeral) ───►│
                      │◄────── HELLO_ACK (PC Static + Ephemeral) ───│
                      │                                             │
                      │─────── PROOF (HMAC(auth, "phone" || T)) ───►│
                      │◄────── PENDING (Code: HMAC(K, "code" || T)) │
                      │◄────── WELCOME (HMAC(auth, "pc" || T)) ─────│
```

### 1. Pairing Key ($K$)
When two devices interact for the first time, they compute the long-term pairing secret:
$$K = \text{HKDF-Extract}(\text{salt} = \text{"owlmic pair v3"}, \text{ikm} = \text{ECDH}(\text{phoneStaticPub}, \text{pcStaticPub}))$$
$$\text{PairKey} = \text{HKDF-Expand}(K, \text{info} = \text{phoneId} \parallel \text{pcId}, 32)$$

### 2. Transcript Hash ($T$)
The exact binary payload bytes of `HELLO` and `HELLO_ACK` are hashed to form the transcript:
$$T = \text{SHA-256}(\text{HELLO payload} \parallel \text{HELLO\_ACK payload})$$

### 3. Session Master & Directional Keys
Using the nonces exchanged in `HELLO` and `HELLO_ACK`:
$$\text{sessionMaster} = \text{HKDF}(\text{salt} = \text{phoneNonce} \parallel \text{pcNonce}, \text{ikm} = \text{PairKey} \parallel \text{ECDH}(\text{phoneEph}, \text{pcEph}), \text{info} = \text{"owlmic session v3"})$$

From `sessionMaster`, three 32-byte keys are derived via HKDF-Expand:
- `"phone->pc"`: AES-256-GCM key for phone transmission.
- `"pc->phone"`: AES-256-GCM key for PC transmission.
- `"auth"`: HMAC proof key.

### 4. Proofs & 4-Digit Approval Code
- **Phone Proof**:
  $$\text{mac} = \text{HMAC-SHA256}(\text{auth}, \text{"phone"} \parallel T)$$
- **PC Proof**:
  $$\text{mac} = \text{HMAC-SHA256}(\text{auth}, \text{"pc"} \parallel T)$$
- **4-Digit Approval Code**:
  $$\text{code} = (\text{BigEndianUInt32}(\text{HMAC-SHA256}(K, \text{"code"} \parallel T)[0..4])) \pmod{10000}$$
  Rendered on screen formatted with leading zeros as 4 digits (e.g., `0492`).

---

## 3. Wire Encryption Rules

- **Wired Links (USB Debugging / Tethering)**: Transport packets travel over direct physical bus connections and are not sealed with AES-GCM to minimize CPU and latency. Authentication occurs during the initial handshake.
- **Wireless Links (Wi-Fi / Bluetooth)**: All frames after `WELCOME` are sealed with AES-256-GCM:
  - **Media Packet Nonce**: `stream (1 byte) || 0x00 0x00 0x00 || seq (64-bit uint BE)`.
  - **Control Frame Nonce**: `0xFF (1 byte) || 0x00 0x00 0x00 || counter (64-bit uint BE)`.
  - **Additional Authenticated Data (AAD)**: The packet header is passed as AAD to prevent header tampering.

---

## 4. Replay Protection & Rate Limits

- **Replay Window**: Receivers maintain a 64-packet bitmask replay window. Out-of-order packets older than 64 frames or duplicate sequence numbers are discarded immediately.
- **Sequence Number Exhaustion**: Sequence numbers never wrap. Before a 32-bit sequence counter overflows, the transport is cleanly re-negotiated to generate fresh ephemeral keys.
- **Abuse & DoS Mitigation**:
  - Probe Answer Rate: At most 20 probe answers per second per network interface.
  - Handshake Rate: At most 4 handshakes per second per remote IP.
