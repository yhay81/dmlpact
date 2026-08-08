# DMLPact artifact compatibility corpus

This corpus freezes sealed plans and hash-linked receipts from every published
DMLPact release. Current and future offline readers must accept every
digest-pinned versioned artifact. The v0.1 corpus also carries the adversarial
mutation suite shared by the unchanged v1 plan and receipt schemas.

All database identities and hashes are synthetic. No credentials, production
data, or third-party content are present.

For an intentional contract change, preserve the old directory byte-for-byte,
add a new version, retain an old reader or provide a no-clobber migration, and
document the compatibility decision.
