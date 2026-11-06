-- Password reset by emailed 6-digit code (one active code per person).
create table password_resets (
  user_id    uuid primary key references users(id) on delete cascade,
  code_hash  text not null,
  expires_at timestamptz not null,
  attempts   int not null default 0,
  created_at timestamptz not null default now()
);

-- Invites are either for the partner (a second person, 48 h) or for linking one more of your
-- OWN devices (15 min, redeemable only by the account that created it).
alter table invites add column kind text not null default 'partner' check (kind in ('partner', 'device'));

-- 'bootstrap' = the one-time encrypted snapshot that brings a newly linked device up to date
-- (not charged to the monthly media allowance).
alter table blobs add column purpose text not null default 'media' check (purpose in ('media', 'bootstrap'));
