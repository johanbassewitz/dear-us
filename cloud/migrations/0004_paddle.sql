-- Paddle: the customer behind a subscription (for the "manage subscription" portal).
alter table subscriptions add column customer_id text;
