// Privacy-safe operational alerts for the desktop support plane (D2-E Task 8).
// Queries reference only allowlisted low-cardinality dimensions and safe
// error codes; no query returns request, payload, or identity material.

@description('Log Analytics workspace resource id backing Application Insights.')
param logAnalyticsWorkspaceId string

@description('Action group receiving every support-plane alert.')
param actionGroupId string

@description('Deployment location for the alert rules.')
param location string = resourceGroup().location

@description('Environment tag value (for example prod or staging).')
param environment string

var alertTags = {
  workload: 'desktop-support'
  environment: environment
}

var scheduledAlerts = [
  {
    name: 'desktop-support-authentication-spike'
    description: 'Authentication denial rate spiked.'
    severity: 2
    query: 'AppMetrics | where Name == "sapphirus.support.authentication.outcomes" | where tostring(Properties["outcome"]) == "denied" | summarize denied = sum(Sum) by bin(TimeGenerated, 5m)'
    threshold: 50
  }
  {
    name: 'desktop-support-consent-replays'
    description: 'Consent replay attempts observed.'
    severity: 2
    query: 'AppMetrics | where Name == "sapphirus.support.replays.observed" | summarize replays = sum(Sum) by bin(TimeGenerated, 15m)'
    threshold: 10
  }
  {
    name: 'desktop-support-receipt-signing-failures'
    description: 'Receipt signing dependency failures observed.'
    severity: 1
    query: 'AppMetrics | where Name == "sapphirus.support.dependency.latency" | where tostring(Properties["dependency"]) == "signing" and tostring(Properties["outcome"]) == "failed" | summarize failures = count() by bin(TimeGenerated, 5m)'
    threshold: 1
  }
  {
    name: 'desktop-support-sql-saturation'
    description: 'Authority SQL latency is saturating.'
    severity: 2
    query: 'AppMetrics | where Name == "sapphirus.support.dependency.latency" | where tostring(Properties["dependency"]) == "sql" | summarize p95 = percentile(Sum, 95) by bin(TimeGenerated, 5m) | where p95 > 500'
    threshold: 0
  }
  {
    name: 'desktop-support-model-throttling'
    description: 'Model provider throttling observed.'
    severity: 2
    query: 'AppMetrics | where Name == "sapphirus.support.provider.statuses" | where tostring(Properties["provider_status_class"]) == "429" | summarize throttles = sum(Sum) by bin(TimeGenerated, 5m)'
    threshold: 5
  }
  {
    name: 'desktop-support-privacy-canary'
    description: 'A privacy canary marker reached exported telemetry. Sev 0: engage the incident procedure.'
    severity: 0
    query: 'union AppTraces, AppExceptions, AppRequests, AppDependencies | where tostring(Properties) contains "CANARY_" or Message contains "CANARY_" | summarize hits = count() by bin(TimeGenerated, 5m)'
    threshold: 0
  }
  {
    name: 'desktop-support-slo-burn'
    description: 'Availability SLO fast burn: server failure ratio over 5 minutes.'
    severity: 1
    query: 'AppRequests | summarize total = count(), failed = countif(ResultCode startswith "5") by bin(TimeGenerated, 5m) | where total > 0 | extend ratio = todouble(failed) / todouble(total) | where ratio > 0.05'
    threshold: 0
  }
]

resource alertRules 'Microsoft.Insights/scheduledQueryRules@2023-12-01' = [
  for alert in scheduledAlerts: {
    name: alert.name
    location: location
    tags: alertTags
    properties: {
      description: alert.description
      displayName: alert.name
      severity: alert.severity
      enabled: true
      evaluationFrequency: 'PT5M'
      windowSize: 'PT5M'
      scopes: [logAnalyticsWorkspaceId]
      criteria: {
        allOf: [
          {
            query: alert.query
            timeAggregation: 'Count'
            operator: 'GreaterThan'
            threshold: alert.threshold
            failingPeriods: {
              numberOfEvaluationPeriods: 1
              minFailingPeriodsToAlert: 1
            }
          }
        ]
      }
      actions: {
        actionGroups: [actionGroupId]
      }
      autoMitigate: true
    }
  }
]

output alertRuleNames array = [for alert in scheduledAlerts: alert.name]
