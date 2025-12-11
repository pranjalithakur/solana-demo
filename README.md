## Solana Vulnerable Demo DApp

This repository is a **deliberately vulnerable Solana / Anchor project** intended for education and tooling evaluation (auditors, scanners, students).

- **Stack**: Anchor (Rust on-chain program) + minimal TypeScript test harness.
- **Program**: `vuln_dapp` – combines a staking pool, an escrow marketplace, and a SOL treasury.
- **NOTE**: Do **not** deploy this to any network with real value. It is insecure by design.

---

### Project Structure

- **`Cargo.toml`**: Workspace definition including the `vuln_dapp` Anchor program.
- **`Anchor.toml`**: Anchor configuration (localnet program ID, provider, scripts).
- **`programs/vuln_dapp`**:
  - **`Cargo.toml`**: Program crate definition and dependencies.
  - **`src/lib.rs`**: Main Anchor program with multiple features and intentionally introduced vulnerabilities.
- **`tests/vuln_dapp.ts`**: Minimal Anchor test scaffold (realistic structure, but logic kept short on purpose).
- **`package.json`**: TypeScript / Anchor test tooling.

---

### High-Level Features

- **Staking Pool**
  - Users "stake" SPL tokens into a shared pool vault.
  - Tracks each user in a `UserState` account.
  - Supports configurable lock-up period before unstaking.

- **Escrow Marketplace**
  - Makers deposit tokens into an escrow vault to create an offer.
  - Separate PDA signer controls the escrow vault.
  - Anyone can (incorrectly) cancel and redirect escrow funds.

- **SOL Treasury**
  - PDA that can hold SOL.
  - Instruction to withdraw SOL from the treasury to an arbitrary recipient.

All of the above are implemented in a way that mimics real-world DeFi / DApp patterns, but with **critical security flaws**.

---

### Easy-to-Detect Vulnerabilities (Beginner-Friendly)

- **Missing / incorrect authority checks**
  - `initialize_pool` allows **any caller** to set an arbitrary `admin` on the `Pool` account, without restricting it to `payer` or a specific authority.
  - `set_lock_seconds` can be called by **anyone** to change the lock duration for all stakers; there is no admin check.
  - `emergency_drain` performs a full token vault drain with **no admin or signer check** on who is allowed to call it.
  - `cancel_escrow` allows **any caller** to cancel an escrow and send funds to any `recipient` token account.

- **Unchecked SPL Token account invariants**
  - In several instructions (e.g. `stake`, `create_escrow`) there is **no validation** that:
    - `user_token_account` / `maker_token_account` belong to the calling user.
    - Token accounts have the expected `mint`.
    - The token account `owner` is the expected authority.
  - This allows attackers to route someone else’s token account into the flow.

- **Client-supplied timestamps**
  - `stake` and `unstake` accept `client_now_ts` from the client instead of reading the on-chain clock.
  - Lock enforcement in `unstake` can be bypassed by supplying a forged timestamp.
  - `create_escrow` uses `expires_at_client_ts` directly, with no on-chain time or sanity check.

---

### More Subtle / Complex Vulnerabilities

- **Insecure PDA seed design**
  - Multiple signer PDAs (`pool_signer`, `escrow_signer`, `treasury_signer`) are derived from **weak or overly-shared seeds**:
    - `pool_signer` uses only `[b"pool"]` and a single `bump`.
    - `escrow_signer` uses only `[b"escrow", mint]`, so **all escrows for the same mint share a single signer**.
    - `treasury_signer` uses only `[b"treasury"]`.
  - These signers are then used to:
    - Move SPL tokens from shared vaults.
    - Transfer SOL from the treasury via `invoke_signed`.
  - While on-chain derivation is still collision-resistant, the **lack of additional discriminating seeds** (e.g. user keys, pool IDs, markets) means a single compromised authority or logic bug can impact many accounts at once, and the design invites misuse of a "global admin PDA".

- **Authority / state separation mistakes**
  - `Pool` contains an `admin` field but **no instruction checks** actually use it to gate privileged operations.
  - `WithdrawSolFromTreasury` does not require or validate any relationship between the caller and the `Treasury` account – anyone can call it to move SOL out.

- **Escrow state re-use & signer coupling**
  - `escrow` and `escrow_signer` share the same `[b"escrow", mint]` seeds:
    - All escrows for a given mint re-use the same signer PDA.
    - Cancel / drain logic treats this signer as the sole authority for the vault.
  - This pattern is subtly dangerous because:
    - A single `escrow` account’s state can be used to justify moving all tokens from **any vault** controlled by the same signer PDA.
    - Bugs or missing checks (like in `cancel_escrow`) become **multi-tenant catastrophes** instead of isolated position failures.

- **Lamports (SOL) movement with unchecked authority**
  - `withdraw_sol_from_treasury` uses `invoke_signed` with `[b"treasury"]` seeds and **no other checks**, enabling:
    - Arbitrary withdrawals of SOL from `treasury_signer` to any `recipient`.
    - No confirmation that the caller is the intended admin or that the withdrawal respects any policy.

- **Global mutable parameters**
  - `lock_seconds` is stored **once** on the `Pool` and can be mutated globally via `set_lock_seconds`:
    - A malicious actor can set it to `0` to allow instant unstaking for everyone.
    - Or set it to a very large number to grief users (permanent lock).
  - This pattern is realistic in DeFi protocols with misdesigned governance or missing access control.

---

### How to Use This Repository

- **Install dependencies**

```bash
cd solana_demo
npm install
```

- **Run localnet & tests** (assuming you have Solana + Anchor installed)

```bash
anchor test
```

> The tests are intentionally minimal; the main focus is the on-chain Rust code and its vulnerabilities.

---

### Ideas for Exercises / Tools

- **Manual review**
  - Identify all instructions and classify their vulnerabilities (auth, PDA design, time, token invariants, etc.).
  - Propose secure redesigns for each major feature (staking, escrow, treasury).

- **Static / dynamic analysis**
  - Run your Solana security scanner, lints, or custom scripts against `programs/vuln_dapp/src/lib.rs`.
  - Evaluate how many issues are detected automatically vs those that require human reasoning.

---

### Disclaimer

This code is **for educational purposes only**.  
Do not deploy it to mainnet or any environment where it could hold real value.  
You are responsible for using it safely and ethically.
