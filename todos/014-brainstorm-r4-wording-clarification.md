---
title: "docs: clarify R4 controversial selection wording in brainstorm requirements"
priority: low
milestone: v0.3
depends_on: 004-verb-brainstorm
---

# Clarify R4 Wording in Brainstorm Requirements

## Problem

R4 in `docs/brainstorms/2026-03-30-brainstorm-verb-requirements.md` says "answers that score high overall BUT have high evaluator disagreement" — but mathematically, high disagreement (e.g., scores [1, 10]) produces a mid-range mean (~5.5), not "high overall."

The Key Decision section correctly describes it as "An answer half the evaluators love and half dislike is more interesting than one everyone rates 7" — which is about variance, not high mean.

The implementation (`mean * stddev`) is correct. Only the requirement text is imprecise.

## Fix

Revise R4 to: "Selection uses a 'controversial' algorithm: answers with high evaluator disagreement (measured by score variance) are prioritized, with preference for higher average scores when disagreement is equal."

## Origin

CodeRabbit CLI review finding (P2).
