-- Reports about Cupid's (AI) answers, sent by people from the app. The reporter chooses
-- whether the answer's text is included; nothing else from the chat is sent.
create table ai_reports (
  id         uuid primary key,
  user_id    uuid references users(id) on delete set null,
  space_id   uuid references spaces(id) on delete set null,
  reason     text not null check (reason in ('harmful', 'inappropriate', 'wrong', 'other')),
  details    text check (char_length(details) <= 2000),
  answer     text check (char_length(answer) <= 8000),
  created_at timestamptz not null default now(),
  handled_at timestamptz
);
