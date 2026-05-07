export interface AppViewEntitySet {
  name: string;
  label: string;
  columns: string[];
  orderby?: string;
  top?: number;
}

export interface AppViewAction {
  label: string;
  kind: 'patrol-run';
  patrolKind: string;
  requiredCapabilities: string;
}

export interface AppViewManifest {
  name: string;
  title: string;
  summary: string;
  entitySets: AppViewEntitySet[];
  actions: AppViewAction[];
  timeline: Array<{ from: string; to: string; via: string }>;
  proofEntitySet: string;
}

export const pawPatrolView: AppViewManifest = {
  name: 'paw-patrol',
  title: 'Paw Patrol',
  summary: 'Risk Patrol intake, worker execution, independent review, evaluation, and proof state.',
  actions: [
    {
      label: 'Run Datadog Patrol',
      kind: 'patrol-run',
      patrolKind: 'datadog_observability',
      requiredCapabilities: 'datadog_query'
    }
  ],
  entitySets: [
    { name: 'WorkRequests', label: 'Work Requests', columns: ['Status', 'Source', 'RequestText', 'FactoryCaseId'], top: 12 },
    { name: 'Signals', label: 'Signals', columns: ['Status', 'Source', 'Severity', 'Summary', 'FactoryCaseId'], top: 12 },
    { name: 'PatrolRuns', label: 'Patrol Runs', columns: ['Status', 'PatrolKind', 'Summary', 'WorkerRunId'], top: 12 },
    { name: 'ObservabilityFindings', label: 'Observability Findings', columns: ['Status', 'Severity', 'RiskLane', 'Title', 'WorkCycleId'], top: 12 },
    { name: 'FactoryCases', label: 'Factory Cases', columns: ['Status', 'MinimumRiskLane', 'Summary', 'WorkCycleId'], top: 12 },
    { name: 'WorkCycles', label: 'Work Cycles', columns: ['Status', 'RiskLane', 'TaskSummary', 'ImplementerWorkerRunId', 'ReviewerRunId', 'EvaluationRunId'], top: 12 },
    { name: 'WorkerRuns', label: 'Worker Runs', columns: ['Status', 'RunnerKind', 'AllowedWorkerId', 'RequiredCapabilities', 'BranchName'], top: 12 },
    { name: 'ReviewRuns', label: 'Review Runs', columns: ['Status', 'ReviewerId', 'Verdict', 'WorkerRunId'], top: 12 },
    { name: 'EvaluationRuns', label: 'Evaluation Runs', columns: ['Status', 'EvaluatorId', 'RequiredChecks', 'WorkCycleId'], top: 12 },
    { name: 'ProofPackets', label: 'Proof Packets', columns: ['Status', 'WorkCycleId', 'ReviewerVerdict', 'ResidualRisks'], top: 12 },
    { name: 'WorkerAgents', label: 'Worker Agents', columns: ['Status', 'WorkerId', 'ProviderId', 'Capabilities', 'LastSeenAt'], top: 12 }
  ],
  timeline: [
    { from: 'WorkRequests', to: 'FactoryCases', via: 'work_request_id' },
    { from: 'Signals', to: 'FactoryCases', via: 'signal_id' },
    { from: 'PatrolRuns', to: 'ObservabilityFindings', via: 'patrol_run_id' },
    { from: 'ObservabilityFindings', to: 'WorkCycles', via: 'work_cycle_id' },
    { from: 'FactoryCases', to: 'WorkCycles', via: 'work_cycle_id' },
    { from: 'WorkCycles', to: 'WorkerRuns', via: 'implementer_worker_run_id' },
    { from: 'WorkerRuns', to: 'ReviewRuns', via: 'worker_run_id' },
    { from: 'WorkCycles', to: 'EvaluationRuns', via: 'evaluation_run_id' },
    { from: 'WorkCycles', to: 'ProofPackets', via: 'proof_packet_id' }
  ],
  proofEntitySet: 'ProofPackets'
};
