# The private $PRIV/env.sh, by name only (ADR-012 D12.6). Copy this file to
# <checkout>/.cache/ops/beta/env.sh (mode 0600) and fill in the values there.
# Never commit values: they identify the account, stack and its resources.
# ops/beta/lib.sh sources the private copy; the scripts print variables by
# name and never expand them into output.
#
# These are the 22 variables of the APP-078 window's env.sh. Four are now
# derived from the checkout by ops/beta/lib.sh and must NOT be set here (a value
# would be ignored): REPO, CAND, PRIV, OPVENV. Five were per-window values of
# that window and belong in $PRIV/windows/<id>/release.json now: CANDIDATE,
# WINDOW, WINDOW2, QUALIFICATION2_DIGEST, TRIAL_START_MS (lib.sh unsets the first
# four; they are listed for the record).

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
# REPO CAND PRIV OPVENV

# Per-window values of the APP-078 window: now in windows/<id>/release.json
# CANDIDATE WINDOW WINDOW2 QUALIFICATION2_DIGEST TRIAL_START_MS
