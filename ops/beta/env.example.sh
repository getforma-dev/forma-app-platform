# The private $PRIV/env.sh, by name only (ADR-012 D12.6). Copy this file to
# <main checkout>/.cache/ops/beta/env.sh (mode 0600), shared by its worktrees,
# and fill in the values there. `just deploy help` prints the resolved path;
# an explicit caller-supplied GENUI_OPS_PRIV may override it.
# Never commit values: they identify the account, stack and its resources.
# ops/beta/lib.sh sources the private copy; the scripts print variables by
# name and never expand them into output.
#
# These are the variables of the APP-078 window's env.sh. Code/environment
# paths and private state are now derived by ops/beta/lib.sh and must NOT be
# set here (a value would be ignored): REPO, CAND, PRIV, OPVENV, PY, GENUI_OPS_PRIV.
# Five were per-window values of
# that window and belong in $PRIV/windows/<id>/release.json now: CANDIDATE,
# WINDOW, WINDOW2, QUALIFICATION2_DIGEST, TRIAL_START_MS (lib.sh unsets the first
# three; they are listed for the record).

# Stack identity (read by ops/beta/deploy.sh and run-step.sh)
export AWS_REGION=          # the region of the stack
STACK=                      # the CloudFormation stack name (GenuiSyntheticFoundation)
REG=                        # the ECR registry host (<account>.dkr.ecr.<region>.amazonaws.com)
CLUSTER=                    # the ECS cluster
SERVICE=                    # the ECS service
DB=                         # the RDS instance identifier (snapshots, 5.1)
LOGS=                       # the CloudWatch log group (operator task logs)
ASSETS=                     # the CloudFormation template bucket (change sets, 3.4)
TG=                         # the ALB target group (health, 6.1)
INPUT_SECRET=               # the operator invitation input secret
SNAP=                       # the APP-078 window's snapshot id (history)
ROLL=                       # the deny-all rollback task definition at APP-078 (history; deploy reads the window's stack outputs)
CUR_TD=                     # the task definition live at APP-078 (history)

# Derived by ops/beta/lib.sh: do not set
# REPO CAND PRIV OPVENV PY GENUI_OPS_PRIV

# Per-window values of the APP-078 window: now in windows/<id>/release.json
# CANDIDATE WINDOW WINDOW2 QUALIFICATION2_DIGEST TRIAL_START_MS
