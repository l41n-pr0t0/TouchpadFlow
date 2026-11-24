# Security Policy

## Reporting a Vulnerability

TouchpadFlow intercepts low-level hardware input events and interacts directly with system privileges via udev `uaccess`. We take input privacy, privilege boundaries, and interface security seriously.

If you discover a security vulnerability, please **do not open a public issue**. Instead, report it responsibly:

- Email: `security@touchpadflow.org` or contact repository maintainers directly.
- Include a detailed description of the vulnerability, steps to reproduce, and affected platforms.
- We will acknowledge receipt within 48 hours and work with you on a coordinated disclosure timeline.

## Security Architecture & Guarantees

1. **Least Privilege:**
   - TouchpadFlow **never runs as root**.
   - Session access is dynamically bounded via `systemd-logind` and udev `uaccess`.
   - Never add users to the broad `input` group, which exposes keyboard events.
2. **Input Privacy:**
   - TouchpadFlow exclusively processes multitouch coordinate axes (`ABS_MT_POSITION_X/Y`).
   - It **never intercepts or logs keystrokes**.
   - Ephemeral memory: coordinates are held only in stack buffers and discarded upon contact release.
   - Network isolation: application and systemd user services enforce `IPAddressDeny=any`.
3. **Control Center Web Security:**
   - Local interface binds strictly to `127.0.0.1`.
   - Cryptographic session tokens are required for all configuration and service control requests.
   - `Host` and `Origin` headers are validated against CSRF attacks.
