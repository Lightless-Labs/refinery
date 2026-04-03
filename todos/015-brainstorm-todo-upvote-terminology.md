---
title: "docs: clarify upvotes/downvotes terminology in brainstorm TODO"
priority: low
milestone: v0.3
---

# Clarify Upvotes/Downvotes Terminology

## Problem

`todos/004-verb-brainstorm.md` line 33 uses binary "upvotes/downvotes" terminology from Reddit's Controversial algorithm, but the actual implementation uses continuous numerical scores (1-10) with standard deviation.

## Fix

Rephrase the "Controversy score" bullet to describe the formula in terms of numerical evaluator scores rather than binary votes, or clarify the mapping.

## Origin

CodeRabbit CLI review finding (P3).
