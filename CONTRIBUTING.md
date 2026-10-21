# LiquiVault Contribution Guide

We welcome contributions from the community! Below is a quick start guide to get you up and running.

## Getting Started
1. **Fork the repository** on GitHub.
2. **Clone your fork**:
   ```bash
   git clone https://github.com/<your‑username>/<repo>.git
   cd <repo>
   ```
3. **Create a new branch** for your work:
   ```bash
   git checkout -b my-feature
   ```
4. **Install dependencies**:
   - **Contracts** (`liquivault-contracts`):
     ```bash
     rustup component add rustfmt clippy
     cargo build
     ```
   - **SDK** (`liquivault-sdk`):
     ```bash
     npm ci
     ```
   - **Frontend** (`liquivault-frontend`):
     ```bash
     npm ci
     cp .env.example .env.local   # adjust as needed
     ```

## Development Workflow
- **Run tests**
  - Contracts: `cargo test --workspace`
  - SDK: `npm test`
  - Frontend: `npm test`
- **Code formatting & linting**
  - Contracts: `cargo fmt -- --check && cargo clippy -- -D warnings`
  - SDK & Frontend: `npm run lint` (uses `eslint` / `prettier`)
- **Build**
  - SDK: `npm run build`
  - Frontend: `npm run build`
- **Commit**
  ```bash
  git add .
  git commit -m "feat: short description"
  ```
- **Push & open a Pull Request**
  ```bash
  git push origin my-feature
  ```
  Then open a PR on GitHub targeting `main`.

## Review Checklist (for maintainers)
- ✅ All tests pass.
- ✅ CI pipeline succeeds.
- ✅ No linting errors.
- ✅ Changes follow the project’s coding style.
- ✅ Documentation updated if applicable.

## Helpful Resources
- **Stellar Soroban Docs:** https://soroban.stellar.org/docs
- **Drips Wave Guidelines:** https://drips.network/wave-guidelines
- **Contributor Covenant Code of Conduct:** see `CODE_OF_CONDUCT.md`
