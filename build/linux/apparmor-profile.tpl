abi <abi/4.0>,
include <tunables/global>

# Lets Lumen's Electron binary use unprivileged user namespaces for Chromium's sandbox.
profile ${executable} /opt/${sanitizedProductName}/${executable}-bin flags=(unconfined) {
  userns,

  # Site-specific additions and overrides. See local/README for details.
  include if exists <local/${executable}>
}
