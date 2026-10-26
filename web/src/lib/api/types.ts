// The API's types, generated from openapi.json (see scripts and DESIGN.md §7).
import type { components } from "./schema";

type Schemas = components["schemas"];

export type JobRequest = Schemas["JobRequest"];
export type Instance = Schemas["Instance"];
export type Part = Schemas["Part"];
export type Panel = Schemas["Panel"];
export type Saw = Schemas["Saw"];
export type Objective = Schemas["Objective"];
export type Stages = Schemas["Stages"];
export type CutType = Schemas["CutType"];
export type FirstCuts = Schemas["FirstCuts"];
export type JobStatus = Schemas["JobStatus"];
export type JobState = Schemas["JobState"];
export type EndedBy = Schemas["EndedBy"];
export type PlanReport = Schemas["PlanReport"];
export type StateEvent = Schemas["StateEvent"];
export type Metrics = Schemas["Metrics"];
export type Solution = Schemas["Solution"];
export type Pattern = Schemas["Pattern"];
export type PlanNode = Schemas["Node"];
export type CutDirection = Schemas["CutDirection"];
export type ErrorBody = Schemas["ErrorBody"];
export type FieldProblem = Schemas["FieldProblem"];

/** Job states after which nothing changes any more. */
export const FINAL_STATES: readonly JobState[] = ["done", "cancelled", "failed"];

export function isFinal(state: JobState): boolean {
  return FINAL_STATES.includes(state);
}
