-- P2 + P3: email notifications, Akismet, reactions.

alter table comment_settings
    add column if not exists notify_email text,
    add column if not exists akismet_key text;

-- One like per (comment, client). The IP is best-effort behind the proxy, so
-- dedup is best-effort too; the primary key still stops trivial double-clicks.
create table if not exists comment_reaction (
    comment_id bigint not null references comment (id) on delete cascade,
    ip text not null,
    created_at timestamptz not null default now(),
    primary key (comment_id, ip)
);
