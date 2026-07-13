# Windows authentication broker

This directory is a frozen D2 design scaffold. It is not built, launched, packaged, or shipped by
the current D1 desktop slice. When the toolchain lane is explicitly resumed, the intended
self-contained helper will own MSAL/WAM broker and cache operations only. The intended Rust host
integration creates a current-user named pipe, launches an Authenticode-signed helper with only the
random pipe name, verifies the helper identity, and receives a short-lived access token in memory.
Refresh material, raw MSAL errors, workspace paths, local-store data, and provider credentials must
never cross the pipe.

System-browser authorization-code plus PKCE fallback is accepted only when WAM is unavailable and
the signed tenant policy explicitly permits it.

Interactive requests carry the desktop window's HWND as a bounded hexadecimal protocol field; it
is supplied to WAM so account UI is parented to the trusted desktop window. The helper rejects a
missing/zero handle, duplicate JSON keys, unknown properties, non-tenant authorities, and malformed
account or scope values before MSAL is invoked. Organization tenant, desktop client, and API-resource
allowlists still need to be sealed into the signed internal package before connected D2 is enabled.
