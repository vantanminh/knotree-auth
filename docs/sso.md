# SSO

Knotree services do not share a cookie. After a user has a session on `accounts.knotree.com`, a service starts the authorization-code flow. If the session is still valid, the user is not asked for a password again. The service receives its own tokens and keeps its own application session.

A compromised sibling such as `foo.knotree.com` cannot read the accounts session cookie and is not an OAuth client until it is registered with an exact redirect URI.

See [How to integrate a service](integrating-a-service.md).
