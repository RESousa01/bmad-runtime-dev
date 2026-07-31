export function isValidAzureOpenAiEndpoint(value: string): boolean {
  try {
    const url = new URL(value);
    if (
      url.protocol !== "https:"
      || url.username !== ""
      || url.password !== ""
      || url.search !== ""
      || url.hash !== ""
      || url.port !== ""
    ) {
      return false;
    }

    const host = url.hostname.toLowerCase();
    const validHost = host.endsWith(".openai.azure.com")
      || host.endsWith(".cognitiveservices.azure.com");
    const validPath = url.pathname === "/"
      || url.pathname === "/openai/v1"
      || url.pathname === "/openai/v1/";

    return validHost && validPath;
  } catch {
    return false;
  }
}
