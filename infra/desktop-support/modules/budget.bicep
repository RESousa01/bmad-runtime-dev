// Resource-group budget with action-group notifications (D2-E Task 10).

@description('Budget resource name.')
param budgetName string

@description('Monthly amount in the subscription currency.')
param amount int

@description('Action group notified at the alert thresholds.')
param actionGroupId string

resource budget 'Microsoft.Consumption/budgets@2024-08-01' = {
  name: budgetName
  properties: {
    category: 'Cost'
    amount: amount
    timeGrain: 'Monthly'
    timePeriod: {
      startDate: '2026-08-01'
    }
    notifications: {
      actual80: {
        enabled: true
        operator: 'GreaterThan'
        threshold: 80
        contactEmails: []
        contactGroups: [actionGroupId]
      }
      forecast100: {
        enabled: true
        operator: 'GreaterThan'
        threshold: 100
        thresholdType: 'Forecasted'
        contactEmails: []
        contactGroups: [actionGroupId]
      }
    }
  }
}
