Type: grilling
Status: open (deferred — see ticket 10)

## Decision (deferred)

Per the analysis in ticket 10, `is_read_only` and `check_permissions`
are different concerns and should not be merged. The right direction
is a small delegation: `is_read_only` should consult
`check_permissions` for the strict mode. See ticket 10 for the
concrete plan and acceptance.
