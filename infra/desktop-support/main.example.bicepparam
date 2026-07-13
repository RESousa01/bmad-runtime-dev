using './main.bicep'

param namePrefix = 'sapphirus-int'
param deployApi = false
param entraAuthority = 'https://login.microsoftonline.com/00000000-0000-0000-0000-000000000000/v2.0'
param entraAudience = 'api://00000000-0000-0000-0000-000000000000'
param sqlAdministratorObjectId = '00000000-0000-0000-0000-000000000000'
param sqlAdministratorLogin = 'Sapphirus SQL Administrators'
param modelName = 'gpt-5'
param modelVersion = 'replace-with-reviewed-version'
param releaseChannel = 'beta'
param tags = {
  application: 'sapphirus'
  distribution: 'internal'
}
