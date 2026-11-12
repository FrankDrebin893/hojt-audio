# Triage Labels

Pocock's skills speak of five triage roles. This repo has its own states (`CLAUDE.md`, "How work
flows"), so the roles map onto them like this. No skill adds a label this repo lacks.

| Role in Pocock's skills | Label here | Note |
| --- | --- | --- |
| `needs-triage` | `state:idea` | Filed, not yet thought through |
| `needs-info` | `needs:reporter` | Waiting on the reporter (`/specwriter` asks and re-checks) |
| `ready-for-agent` | `state:approved` | Set by the owner only. An agent never applies it |
| `ready-for-human` | `needs:engineer` | Awaiting an engineer decision |
| `wontfix` | closed as not planned | With a comment saying why |

`/groom` and `/specwriter` triage with these labels. Pocock's `/triage` is not used in a routine.
