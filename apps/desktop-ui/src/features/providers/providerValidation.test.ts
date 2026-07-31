import { describe, expect, it } from "vitest";

import { isValidAzureOpenAiEndpoint } from "./providerValidation";

describe("isValidAzureOpenAiEndpoint", () => {
  it.each([
    "https://forge-models.openai.azure.com",
    "https://forge-models.openai.azure.com/openai/v1",
    "https://forge-models.openai.azure.com/openai/v1/",
    "https://forge-models.cognitiveservices.azure.com/openai/v1/",
    "https://forge-models.openai.azure.com:443/openai/v1/",
  ])("accepts the native Azure OpenAI v1 endpoint shape: %s", (endpoint) => {
    expect(isValidAzureOpenAiEndpoint(endpoint)).toBe(true);
  });

  it.each([
    "https://example.services.ai.azure.com/api/projects/Forge",
    "http://forge-models.openai.azure.com/openai/v1/",
    "https://user@forge-models.openai.azure.com/openai/v1/",
    "https://forge-models.openai.azure.com/openai/v1/responses",
    "https://forge-models.openai.azure.com/openai/v1/?x=1",
    "https://forge-models.openai.azure.com:8443/openai/v1/",
    "https://example.com/openai/v1/",
  ])("rejects an endpoint the native direct provider rejects: %s", (endpoint) => {
    expect(isValidAzureOpenAiEndpoint(endpoint)).toBe(false);
  });
});
