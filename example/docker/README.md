# How to run using docker compose

## Setup

1. Copy `env.template` to `.env`
2. Customize the credentials and connection strings in `.env`
3. Run with the env file:

## Daemon mode

run:
`docker compose --env-file env.template up -d`

stop:
`docker compose --env-file env.template down`

## Simple mode

run:
`docker compose --env-file env.template up`
