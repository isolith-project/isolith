# Isolith domain: DNS records + production apply (SITE-DOMAIN-ORIGIN)

Status as of 2026-10-27 (verified this card, via RDAP -- the .dev registry
returns "not found" and DNS has no answers): the domain is NOT yet registered.
Don registers it at Porkbun. These are the exact records to add, and the exact
production Caddy block already validated and staged.

Recommended split (from the card): isolith.dev as the product and docs origin;
isolith.org registered defensively in the same order, pointed at the same
records. Everything below writes isolith.dev; isolith.org mirrors the apex
records.

## Registrar settings (Porkbun, on registration)

1. Registrar lock ON (domain lock / transfer lock).
2. WHOIS privacy ON.
3. DNS records below (Porkbun uses its own nameservers by default; paytruthapp.com
   already runs this way, so no nameserver change is needed).

## DNS records (exact)

    Type    Name             Value                      TTL
    ----    ----             -----                      ---
    A       @               46.62.170.182               300
    AAAA    @               2a01:4f9:c012:cc57::1       300
    CNAME   www             isolith.dev.               300
    MX      @               10 fwd1.porkbun.com.        300   (email forward)
    MX      @               10 fwd2.porkbun.com.        300   (email forward)
    TXT     @               v=spf1 include:spf.porkbun.com ~all   300

Use Porkbun's managed email forwarding panel (DNS -> Email Forwarding) to create
the aliases; do not create them as mailbox accounts:

    hello@isolith.dev       ->  forward to Don's existing mailbox
    security@isolith.dev    ->  forward to Don's existing mailbox

That is the happy path of the card: "email forwarding aliases only (hello@ and
security@ to Don's existing mailbox)". No catch-all, no inbound server. The
conduct@ alias used by the Code of Conduct can share the same mailbox.

If Don already has a designated mailbox address different from the one used for
paytruthapp.com forwarding, use that one for both aliases.

## HSTS

HSTS is a web header served by Caddy, not a DNS record. The vhost sets
`Strict-Transport-Security: max-age=31536000; includeSubDomains`. No action
needed in DNS. (No HSTS-preload submission is requested; it is an extra step
that can be added later if wanted.)

## Wait: registrars require NS setup

On registration Porkbun will allocate its four nameservers to the domain
(ns1..ns4). No delegation change is needed; the records above are added inside
Porkbun's DNS panel for the domain.

## Production Caddy block (already validated, already loaded via admin API)

The running Caddy on the Hetzner VPS (46.62.170.182) already serves these vhosts
from `/var/www/isolith` (the static public export, this card):

    isolith.dev {
        encode gzip
        header Strict-Transport-Security "max-age=31536000; includeSubDomains"
        header {
            X-Content-Type-Options "nosniff"
            X-Frame-Options "DENY"
            Referrer-Policy "strict-origin-when-cross-origin"
            -Server
        }
        root * /var/www/isolith
        file_server
        try_files {path} {path}/ {path}.html
        handle /.well-known/acme-challenge/* {
            file_server
        }
    }

    www.isolith.dev {
        redir https://isolith.dev{uri} 301
    }

So the instant the DNS records above resolve to 46.62.170.182, Caddy issues the
Let's Encrypt certificate automatically, HTTPS serves on 443 and the HSTS
header is already on every response. No further server work is needed.

The Caddyfile edit was left in /root/caddyfile-with-isolith on the VPS; to make
the change durable across a Caddy restart, apply it to /etc/caddy/Caddyfile
(the config file section) and reload:

    install -m 0644 /root/caddyfile-with-isolith /etc/caddy/Caddyfile
    systemctl reload caddy
    caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile

## Verification you can run the moment DNS points

    for p in "" getting-started.html admin-guide.html format-matrix.html \
             download.html releases.html license.html security.html \
             contact.html what-it-does-not-do.html sitemap.xml robots.txt; do
        curl -s -o /dev/null -w "%{http_code}  https://isolith.dev/$p\n" \
            https://isolith.dev/$p
    done
    curl -sI https://isolith.dev/ | grep -i strict-transport-security
    openssl s_client -connect isolith.dev:443 -servername isolith.dev \
        </dev/null 2>/dev/null | openssl x509 -noout -issuer -enddate

Expected: every page 200, `Strict-Transport-Security: max-age=31536000;
includeSubDomains`, issuer `Let's Encrypt`.

## The report this card was asked for

No new paid service is required. The VPS, the Caddy installation, and the
static-file serving already exist and are already in use (paytruthapp.com,
support.redonealpha.com); this card adopts the same host for the Isolith
docs/product site. The only cost item is the domain registration itself at
Porkbun (~$10/yr), which Don owns.
