-- ============================================================================
-- Dear Us: sync relay setup (version 1)
-- Paste this whole file into your Supabase project: SQL Editor → New query → Run.
-- Safe to run again. It only creates what's missing.
--
-- What this stores: device names, sizes/timestamps, and ENCRYPTED payloads that are
-- deleted as soon as both PCs have them. Supabase can't read your memories:
-- the key never leaves your two computers.
-- ============================================================================

create extension if not exists pgcrypto with schema extensions;

create table if not exists public.spaces (
  id          uuid primary key,
  created_by  uuid not null references auth.users(id) on delete cascade,
  created_at  timestamptz not null default now()
);

create table if not exists public.members (
  space_id  uuid not null references public.spaces(id) on delete cascade,
  user_id   uuid not null references auth.users(id) on delete cascade,
  joined_at timestamptz not null default now(),
  primary key (space_id, user_id)
);

create table if not exists public.devices (
  id            uuid primary key,
  space_id      uuid not null references public.spaces(id) on delete cascade,
  user_id       uuid not null references auth.users(id) on delete cascade,
  name          text not null check (char_length(name) between 1 and 80),
  registered_at timestamptz not null default now(),
  revoked_at    timestamptz,
  last_seen     timestamptz
);

-- LAN address for direct PC-to-PC transfers (optional speed-up; added in v1.1)
alter table public.devices add column if not exists lan_endpoint text check (char_length(lan_endpoint) <= 60);

create table if not exists public.invites (
  secret_hash text primary key,
  space_id    uuid not null references public.spaces(id) on delete cascade,
  created_by  uuid not null references auth.users(id) on delete cascade,
  expires_at  timestamptz not null,
  used_by     uuid,
  used_at     timestamptz
);

create table if not exists public.envelopes (
  id            uuid primary key,
  space_id      uuid not null references public.spaces(id) on delete cascade,
  seq           bigserial,
  sender_device uuid not null references public.devices(id) on delete cascade,
  recipients    uuid[] not null,
  ciphertext    text not null check (char_length(ciphertext) <= 20000000),
  created_at    timestamptz not null default now()
);
create index if not exists envelopes_by_space_seq on public.envelopes(space_id, seq);

create table if not exists public.envelope_acks (
  envelope_id uuid not null references public.envelopes(id) on delete cascade,
  device_id   uuid not null references public.devices(id) on delete cascade,
  acked_at    timestamptz not null default now(),
  primary key (envelope_id, device_id)
);

create table if not exists public.blobs (
  space_id      uuid not null references public.spaces(id) on delete cascade,
  hash          text not null check (hash ~ '^[0-9a-f]{64}$'),
  size          bigint not null,
  chunk_size    bigint not null,
  chunk_count   int not null,
  sender_device uuid not null references public.devices(id) on delete cascade,
  recipients    uuid[] not null,
  created_at    timestamptz not null default now(),
  primary key (space_id, hash)
);

create table if not exists public.blob_acks (
  space_id  uuid not null,
  hash      text not null,
  device_id uuid not null references public.devices(id) on delete cascade,
  acked_at  timestamptz not null default now(),
  primary key (space_id, hash, device_id),
  foreign key (space_id, hash) references public.blobs(space_id, hash) on delete cascade
);

-- ---------------------------------------------------------------- helpers ---
create or replace function public.is_member(p_space uuid) returns boolean
language sql stable security definer set search_path = public as $$
  select exists (select 1 from members where space_id = p_space and user_id = auth.uid());
$$;

create or replace function public.owns_device(p_device uuid) returns boolean
language sql stable security definer set search_path = public as $$
  select exists (select 1 from devices where id = p_device and user_id = auth.uid() and revoked_at is null);
$$;

-- every recipient that's still active has confirmed
create or replace function public.all_acked(p_recipients uuid[], p_acked uuid[]) returns boolean
language sql stable security definer set search_path = public as $$
  select not exists (
    select 1 from unnest(p_recipients) r
    join devices d on d.id = r and d.revoked_at is null
    where not (r = any(coalesce(p_acked, '{}')))
  );
$$;

-- ------------------------------------------------------------------- RLS ---
alter table public.spaces        enable row level security;
alter table public.members       enable row level security;
alter table public.devices       enable row level security;
alter table public.invites       enable row level security;
alter table public.envelopes     enable row level security;
alter table public.envelope_acks enable row level security;
alter table public.blobs         enable row level security;
alter table public.blob_acks     enable row level security;

drop policy if exists "members read space" on public.spaces;
create policy "members read space" on public.spaces for select using (public.is_member(id));

drop policy if exists "members read members" on public.members;
create policy "members read members" on public.members for select using (public.is_member(space_id));

drop policy if exists "members read devices" on public.devices;
create policy "members read devices" on public.devices for select using (public.is_member(space_id));
drop policy if exists "own device heartbeat" on public.devices;
create policy "own device heartbeat" on public.devices for update using (user_id = auth.uid()) with check (user_id = auth.uid());
-- invites: no direct access at all (functions only)

drop policy if exists "members read envelopes" on public.envelopes;
create policy "members read envelopes" on public.envelopes for select using (public.is_member(space_id));
drop policy if exists "own device sends" on public.envelopes;
create policy "own device sends" on public.envelopes for insert
  with check (public.is_member(space_id) and public.owns_device(sender_device));

drop policy if exists "members read acks" on public.envelope_acks;
create policy "members read acks" on public.envelope_acks for select
  using (exists (select 1 from envelopes e where e.id = envelope_id and public.is_member(e.space_id)));
drop policy if exists "own device acks" on public.envelope_acks;
create policy "own device acks" on public.envelope_acks for insert with check (public.owns_device(device_id));

drop policy if exists "members read blobs" on public.blobs;
create policy "members read blobs" on public.blobs for select using (public.is_member(space_id));
drop policy if exists "own device uploads" on public.blobs;
create policy "own device uploads" on public.blobs for insert
  with check (public.is_member(space_id) and public.owns_device(sender_device));

drop policy if exists "members read blob acks" on public.blob_acks;
create policy "members read blob acks" on public.blob_acks for select using (public.is_member(space_id));
drop policy if exists "own device blob acks" on public.blob_acks;
create policy "own device blob acks" on public.blob_acks for insert
  with check (public.is_member(space_id) and public.owns_device(device_id));

-- ------------------------------------------------------------- functions ---
create or replace function public.create_space(p_space uuid) returns void
language plpgsql security definer set search_path = public as $$
begin
  if auth.uid() is null then raise exception 'not signed in'; end if;
  insert into spaces(id, created_by) values (p_space, auth.uid());
  insert into members(space_id, user_id) values (p_space, auth.uid());
end $$;

create or replace function public.register_device(p_device uuid, p_space uuid, p_name text) returns void
language plpgsql security definer set search_path = public as $$
begin
  if not is_member(p_space) then raise exception 'not a member of this space'; end if;
  insert into devices(id, space_id, user_id, name) values (p_device, p_space, auth.uid(), left(p_name, 80))
  on conflict (id) do update set name = excluded.name, last_seen = now()
    where devices.user_id = auth.uid();
end $$;

create or replace function public.create_invite(p_space uuid, p_secret_hash text) returns void
language plpgsql security definer set search_path = public as $$
begin
  if not is_member(p_space) then raise exception 'not a member of this space'; end if;
  insert into invites(secret_hash, space_id, created_by, expires_at)
  values (p_secret_hash, p_space, auth.uid(), now() + interval '48 hours');
end $$;

-- One-time, 48-hour, and a space holds two people at most.
create or replace function public.redeem_invite(p_secret text) returns uuid
language plpgsql security definer set search_path = public, extensions as $$
declare v_space uuid;
begin
  if auth.uid() is null then raise exception 'not signed in'; end if;
  update invites set used_by = auth.uid(), used_at = now()
   where secret_hash = encode(digest(p_secret, 'sha256'), 'hex')
     and used_by is null and expires_at > now()
  returning space_id into v_space;
  if v_space is null then raise exception 'This invite is invalid, expired, or already used.'; end if;
  if (select count(*) from members where space_id = v_space and user_id <> auth.uid()) >= 2 then
    raise exception 'This space already has two people.';
  end if;
  insert into members(space_id, user_id) values (v_space, auth.uid()) on conflict do nothing;
  return v_space;
end $$;

create or replace function public.revoke_device(p_device uuid) returns void
language plpgsql security definer set search_path = public as $$
begin
  update devices set revoked_at = now() where id = p_device and is_member(space_id);
end $$;

create or replace function public.envelope_delivered(p_id uuid) returns boolean
language sql stable security definer set search_path = public as $$
  select coalesce((
    select all_acked(e.recipients, array(select device_id from envelope_acks a where a.envelope_id = e.id))
      from envelopes e where e.id = p_id and is_member(e.space_id)
  ), true);
$$;

-- Deletes only once every partner device confirmed it. Returns true if it's gone.
create or replace function public.cleanup_envelope(p_id uuid) returns boolean
language plpgsql security definer set search_path = public as $$
begin
  if not envelope_delivered(p_id) then return false; end if;
  delete from envelopes where id = p_id and is_member(space_id);
  return true;
end $$;

create or replace function public.blob_delivered(p_space uuid, p_hash text) returns boolean
language sql stable security definer set search_path = public as $$
  select coalesce((
    select all_acked(b.recipients, array(select device_id from blob_acks a where a.space_id = b.space_id and a.hash = b.hash))
      from blobs b where b.space_id = p_space and b.hash = p_hash and is_member(p_space)
  ), true);
$$;

-- Removes the blob record (the app deletes the stored chunks first). Only when delivered.
create or replace function public.forget_blob(p_space uuid, p_hash text) returns boolean
language plpgsql security definer set search_path = public as $$
begin
  if not blob_delivered(p_space, p_hash) then return false; end if;
  delete from blobs where space_id = p_space and hash = p_hash and is_member(p_space);
  return true;
end $$;

create or replace function public.space_usage(p_space uuid) returns bigint
language sql stable security definer set search_path = public as $$
  select case when is_member(p_space) then
    coalesce((select sum(size) from blobs where space_id = p_space), 0)
    + coalesce((select sum(char_length(ciphertext)) from envelopes where space_id = p_space), 0)
  else 0 end;
$$;

create or replace function public.dear_us_version() returns int language sql immutable as $$ select 1 $$;

grant execute on function public.create_space, public.register_device, public.create_invite, public.redeem_invite,
  public.revoke_device, public.envelope_delivered, public.cleanup_envelope, public.blob_delivered,
  public.forget_blob, public.space_usage, public.dear_us_version to authenticated;
grant execute on function public.dear_us_version to anon;

-- --------------------------------------------------------------- storage ---
insert into storage.buckets (id, name, public, file_size_limit)
values ('relay', 'relay', false, 52428800)
on conflict (id) do nothing;

drop policy if exists "dear us members read chunks" on storage.objects;
create policy "dear us members read chunks" on storage.objects for select
  using (bucket_id = 'relay' and public.is_member(((storage.foldername(name))[1])::uuid));
drop policy if exists "dear us members write chunks" on storage.objects;
create policy "dear us members write chunks" on storage.objects for insert
  with check (bucket_id = 'relay' and public.is_member(((storage.foldername(name))[1])::uuid));
drop policy if exists "dear us members update chunks" on storage.objects;
create policy "dear us members update chunks" on storage.objects for update
  using (bucket_id = 'relay' and public.is_member(((storage.foldername(name))[1])::uuid));
drop policy if exists "dear us members delete chunks" on storage.objects;
create policy "dear us members delete chunks" on storage.objects for delete
  using (bucket_id = 'relay' and public.is_member(((storage.foldername(name))[1])::uuid));
