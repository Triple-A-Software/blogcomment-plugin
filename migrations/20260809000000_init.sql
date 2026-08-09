-- One row per comment. `post_id` references the CMS `post` table (a different
-- database, so no FK is possible — it's validated against the CMS on submit).
-- `parent_id` supports threaded replies (P2); P1 renders flat. `status` gates
-- visibility: only 'approved' comments are shown publicly.
create table if not exists comment (
    id bigint generated always as identity primary key,
    post_id int not null,
    parent_id bigint references comment (id) on delete cascade,
    author_name text not null,
    author_email text,
    user_id uuid,
    body text not null,
    status text not null default 'pending' check (status in ('pending', 'approved', 'spam')),
    created_at timestamptz not null default now(),
    ip text,
    ua text
);

create index if not exists comment_post_idx on comment (post_id, status, created_at);
create index if not exists comment_status_idx on comment (status, created_at desc);

-- Single-row plugin settings.
create table if not exists comment_settings (
    id text primary key default 'settings',
    -- new comments start as 'pending' and stay hidden until approved
    require_moderation boolean not null default true,
    -- offer visitors an (optional) email field, for future notifications
    collect_email boolean not null default true,
    -- optional captcha: 'none' | 'hcaptcha' | 'recaptcha'
    captcha_provider text not null default 'none',
    captcha_site_key text,
    captcha_secret text
);

insert into comment_settings (id) values ('settings') on conflict (id) do nothing;
