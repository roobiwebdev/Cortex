# Message for testers

Copy and send. The install commands are explained in [releasing.md, Install with one command](releasing.md#install-with-one-command).

---

Hi! Thanks for trying Cortex, a screen recorder I'm building. Installing takes one command and about a minute.

**Windows 10 or 11 (64-bit).** Open PowerShell (Start menu, type "PowerShell"), paste this and press Enter:

```
irm https://cortex.roobi.dev/install.ps1 | iex
```

**Mac (macOS 15 or later).** Open Terminal (Spotlight, type "Terminal"), paste this and press Enter:

```
curl -fsSL https://cortex.roobi.dev/install.sh | sh
```

**Linux (x86_64 or arm64).** The same command as the Mac, in a terminal, once the Linux build is out.

Cortex opens when the install finishes. On a Mac it asks for Screen Recording permission the first time you record. Turn Cortex on in System Settings, then reopen it. Cortex updates itself after that.

Please send me anything that breaks or confuses you, with a screenshot if you can. If the install itself fails, copy the text from the window and send me that too.
