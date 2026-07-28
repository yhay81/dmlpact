# DMLPact artifact compatibility corpus

This corpus freezes sealed plans and hash-linked receipts from the published
v0.1 machine contract. Current and future offline readers must accept every
versioned artifact and fail closed on the mutations declared by its manifest.

All database identities and hashes are synthetic. No credentials, production
data, or third-party content are present.

For an intentional contract change, preserve the old directory byte-for-byte,
add a new version, retain an old reader or provide a no-clobber migration, and
document the compatibility decision.
