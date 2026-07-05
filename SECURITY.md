# Security Policy

## 🔒 Supported Versions

NovaX is currently in active development (pre-v1.0). Security fixes will be applied to the latest release only.

| Version | Supported          |
|---------|--------------------|
| 0.1.x   | ✅ Latest patch    |
| < 0.1   | ❌ Not supported   |

## 🛡️ Security Goals

NovaX is designed with security as a first-class concern. Our goals:

1. **Secure by default** — every default configuration is safe; insecure options require explicit opt-in
2. **Compile-time checks** — detect SQL injection, XSS, CSRF, secret leakage in compile-time (planned for v0.3)
3. **No unsafe code** — `unsafe` blocks require explicit `#[allow(unsafe)]` review
4. **Minimal dependencies** — every dependency must be justified and audited
5. **Defense in depth** — multiple layers of protection

## 🐛 Reporting a Vulnerability

If you discover a security vulnerability, please **DO NOT** open a public issue.

### Responsible Disclosure Process

1. **Email:** Send details to `security@novax.example.com` (replace with actual email)
2. **Encrypt:** If possible, encrypt your report with our PGP key (fingerprint: `TODO`)
3. **Include:**
   - Description of the vulnerability
   - Steps to reproduce
   - Affected versions
   - Potential impact
   - Suggested fix (if any)

### Response Timeline

- **Acknowledgment:** Within 48 hours
- **Initial assessment:** Within 5 business days
- **Fix development:** Within 30 days for high-severity issues
- **Public disclosure:** After fix is released, with credit to reporter (unless they prefer to remain anonymous)

### Bug Bounty

We are working on establishing a bug bounty program. For now, we will publicly acknowledge all valid security reports.

## 🔐 Security Best Practices (for users)

When deploying NovaX:

1. **Always use HTTPS** — never expose the server directly over HTTP in production
2. **Set strong secrets** — use environment variables for all secrets, never commit them
3. **Run as non-root** — the Docker image runs as user `novax` (UID 1000) by default
4. **Limit resources** — use Docker resource limits (see `docker-compose.production.yml`)
5. **Keep updated** — always run the latest patch version
6. **Enable logging** — set `RUST_LOG=info` (or `warn` for less verbosity)
7. **Monitor health** — use the `/api/health` endpoint for monitoring
8. **Network isolation** — place behind a reverse proxy (nginx, Caddy, etc.)

## 🚫 What NOT to do

- Do not disable TLS verification
- Do not expose the database to the public internet
- Do not run as root inside the container
- Do not commit secrets (API keys, passwords, tokens) to the repository
- Do not use `DEBUG` log level in production (may leak sensitive data)

## 📋 Security Checklist (for contributors)

Before submitting code, verify:

- [ ] No `unsafe` blocks without `#[allow(unsafe)]` and justification
- [ ] No secrets hardcoded in source
- [ ] No SQL string concatenation (use parameterized queries)
- [ ] No `unwrap()` on user input (use proper error handling)
- [ ] All user input is validated and sanitized
- [ ] Authentication required for sensitive endpoints
- [ ] Authorization checked before privileged operations
- [ ] Sensitive data is not logged

## 🔄 Security Audits

NovaX will undergo regular security audits:
- **Dependency audit:** `cargo audit` runs in CI on every PR
- **Code audit:** Annual third-party audit starting from v1.0
- **Penetration testing:** Planned for v1.0 release

## 📞 Contact

- **Security email:** `security@novax.example.com` (replace)
- **General issues:** [GitHub Issues](https://github.com/amir-helal-ali/novax/issues)
- **Security advisories:** [GitHub Security Advisories](https://github.com/amir-helal-ali/novax/security/advisories)
