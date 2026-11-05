-- Dear Us cloud relay, schema v1.
-- The server never sees memories: envelopes and chunks are sealed with the couple's space key,
-- which only exists on their PCs. What is stored here is accounts, device names, sizes, timestamps,
-- plan/quota counters and the encrypted mailbox (deleted as soon as both PCs have each item).

create table users (
  id            uuid primary key,
  email         text not null unique,
  password_hash text not null,
  created_at    timestamptz not null default now()
);

create table refresh_tokens (
  token_hash text primary key,
  user_id    uuid not null references users(id) on delete cascade,
  expires_at timestamptz not null,
  created_at timestamptz not null default now()
);
create index refresh_tokens_by_user on refresh_tokens(user_id);

create table spaces (
  id                uuid primary key,
  created_by        uuid not null references users(id) on delete cascade,
  created_at        timestamptz not null default now(),
  -- Plan state. The trial starts when the partner joins (pairing), not at sign-up.
  trial_ends_at     timestamptz,
  premium_until     timestamptz,
  -- Monthly counters, reset lazily when the period (YYYY-MM, UTC) changes.
  media_period      text not null default '',
  media_bytes       bigint not null default 0,
  cupid_period      text not null default '',
  cupid_used        int not null default 0,
  -- Top-up arrows: never expire, used after the monthly allowance.
  cupid_bonus       int not null default 0,
  daily_pick_on     date
);

create table members (
  space_id  uuid not null references spaces(id) on delete cascade,
  user_id   uuid not null references users(id) on delete cascade,
  joined_at timestamptz not null default now(),
  primary key (space_id, user_id)
);
create index members_by_user on members(user_id);

create table devices (
  id            uuid primary key,
  space_id      uuid not null references spaces(id) on delete cascade,
  user_id       uuid not null references users(id) on delete cascade,
  name          text not null check (char_length(name) between 1 and 80),
  registered_at timestamptz not null default now(),
  revoked_at    timestamptz,
  last_seen     timestamptz,
  lan_endpoint  text check (char_length(lan_endpoint) <= 60)
);
create index devices_by_space on devices(space_id);

create table invites (
  secret_hash text primary key,
  space_id    uuid not null references spaces(id) on delete cascade,
  created_by  uuid not null references users(id) on delete cascade,
  expires_at  timestamptz not null,
  used_by     uuid,
  used_at     timestamptz
);

create table envelopes (
  id            uuid primary key,
  space_id      uuid not null references spaces(id) on delete cascade,
  seq           bigserial,
  sender_device uuid not null references devices(id) on delete cascade,
  recipients    uuid[] not null,
  ciphertext    text not null check (char_length(ciphertext) <= 20000000),
  created_at    timestamptz not null default now()
);
create index envelopes_by_space_seq on envelopes(space_id, seq);

create table envelope_acks (
  envelope_id uuid not null references envelopes(id) on delete cascade,
  device_id   uuid not null references devices(id) on delete cascade,
  acked_at    timestamptz not null default now(),
  primary key (envelope_id, device_id)
);

create table blobs (
  space_id      uuid not null references spaces(id) on delete cascade,
  hash          text not null check (hash ~ '^[0-9a-f]{64}$'),
  size          bigint not null check (size >= 0),
  chunk_size    bigint not null check (chunk_size > 0 and chunk_size <= 16777216),
  chunk_count   int not null check (chunk_count >= 0 and chunk_count <= 100000),
  sender_device uuid not null references devices(id) on delete cascade,
  recipients    uuid[] not null,
  -- true once the blob's size was charged to the monthly media allowance
  counted       boolean not null default false,
  created_at    timestamptz not null default now(),
  primary key (space_id, hash)
);

create table blob_acks (
  space_id  uuid not null,
  hash      text not null,
  device_id uuid not null references devices(id) on delete cascade,
  acked_at  timestamptz not null default now(),
  primary key (space_id, hash, device_id),
  foreign key (space_id, hash) references blobs(space_id, hash) on delete cascade
);

-- Store subscriptions (one row per store recurrence) and one-off purchases (arrow packs).
create table subscriptions (
  id         text primary key,          -- store's recurrence id
  store      text not null,             -- 'microsoft'
  space_id   uuid not null references spaces(id) on delete cascade,
  user_id    uuid not null references users(id) on delete cascade,
  product    text not null,             -- 'premium_monthly' | 'premium_yearly'
  state      text not null,
  expires_at timestamptz not null,
  updated_at timestamptz not null default now()
);
create index subscriptions_by_space on subscriptions(space_id);

create table purchases (
  transaction_id text primary key,      -- store transaction id: credited exactly once
  store          text not null,
  space_id       uuid not null references spaces(id) on delete cascade,
  user_id        uuid not null references users(id) on delete cascade,
  product        text not null,         -- 'arrows_100' | 'arrows_300'
  arrows         int not null,
  created_at     timestamptz not null default now()
);

-- every recipient that's still active has confirmed
create function all_acked(p_recipients uuid[], p_acked uuid[]) returns boolean
language sql stable as $$
  select not exists (
    select 1 from unnest(p_recipients) r
    join devices d on d.id = r and d.revoked_at is null
    where not (r = any(coalesce(p_acked, '{}')))
  );
$$;
