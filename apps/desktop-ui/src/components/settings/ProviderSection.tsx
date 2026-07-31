import { Button } from "@sapphirus/ui";
import { Check, KeyRound, PlugZap, Unplug } from "lucide-react";
import { useEffect, useState } from "react";
import type {
  ModelAuthStatusProjection,
  ModelProviderConnection,
  ModelProviderId,
  ModelSelection,
} from "../../lib/hostClient";
import { isValidAzureOpenAiEndpoint } from "../../features/providers/providerValidation";
import { ProviderIcon } from "./ProviderIcon";

const providers: ReadonlyArray<{
  readonly id: ModelProviderId;
  readonly name: string;
  readonly setup: string;
}> = [
  {
    id: "openai",
    name: "OpenAI",
    setup: "Connect an API key for this Forge session.",
  },
  {
    id: "azure",
    name: "Azure OpenAI",
    setup: "Connect an Azure endpoint and API key for this session.",
  },
];

/**
 * The provider connection: which model, and the key that reaches it.
 *
 * Why this is its own component
 * -----------------------------
 * It is the only part of Settings that handles a secret, and it is the only
 * part that holds a draft the host can overwrite. Both are reasons to be able
 * to read it, and test it, without the other seven panes in the way: an
 * accidental change to how the key is submitted is invisible in a 900-line file
 * where 800 of those lines are themes, counts and version rows.
 *
 * How the key is handled, and why it is handled that way
 * ------------------------------------------------------
 * The credential submitted is read out of the form with `FormData`, never out
 * of `apiKeyDraft`. That is not a style choice: the app runs inside a WebView
 * whose autofill writes the input's value natively, without the event React
 * needs to update state, so a submit that trusted state would send an empty key
 * after a successful autofill. It is also why the key never travels through a
 * prop or a callback argument on the way in.
 *
 * It leaves in exactly one direction — into the `ModelProviderConnection`
 * handed to `onModelConnect` — and is cleared from both places it can survive
 * as soon as that settles: `setApiKeyDraft("")` for React's copy, `form.reset()`
 * for the DOM's, which autofill wrote to directly. `form` is captured before
 * the await because `event.currentTarget` is null by the time the promise
 * settles.
 *
 * Nothing here ever puts the key in rendered text, in a notice, or in an
 * argument to anything but `onModelConnect`. `providerFormNotice` is a fixed
 * string for that reason — echoing the rejected value would put a secret in the
 * DOM as text, where the "no credential on the page" assertions cannot help
 * because they would be asserting on a value the test itself supplied.
 */
export interface ProviderSectionProps {
  readonly agentStatusLabel: string;
  readonly modelAccessDetail: string;
  readonly modelAccessLabel: string;
  readonly modelAuthStatus: ModelAuthStatusProjection | null;
  readonly modelSelection: ModelSelection;
  readonly modelSelectionBusy: boolean;
  readonly modelSelectionNotice: string | null;
  readonly onModelConnect: (connection: ModelProviderConnection) => Promise<void>;
  readonly onModelDisconnect: () => Promise<void>;
  readonly onModelSelectionChange: (selection: ModelSelection) => Promise<void>;
}

export function ProviderSection({
  agentStatusLabel,
  modelAccessDetail,
  modelAccessLabel,
  modelAuthStatus,
  modelSelection,
  modelSelectionBusy,
  modelSelectionNotice,
  onModelConnect,
  onModelDisconnect,
  onModelSelectionChange,
}: ProviderSectionProps) {
  const [selectedProvider, setSelectedProvider] = useState<ModelProviderId>(
    modelSelection.providerId,
  );
  const [modelDraft, setModelDraft] = useState(modelSelection.modelId);
  const [apiKeyDraft, setApiKeyDraft] = useState("");
  const [providerFormNotice, setProviderFormNotice] = useState<string | null>(null);
  const [azureEndpointDraft, setAzureEndpointDraft] = useState("");
  const [azureRegionDraft, setAzureRegionDraft] = useState("");
  const modelIsValid = /^[A-Za-z0-9][A-Za-z0-9._:/-]{0,127}$/u.test(modelDraft);
  const azureEndpointIsValid = selectedProvider !== "azure"
    || isValidAzureOpenAiEndpoint(azureEndpointDraft);
  const selectedProviderName = selectedProvider === "azure" ? "Azure OpenAI" : "OpenAI";
  const selectedProviderConnected = modelAuthStatus?.status === "development_ready"
    && modelAuthStatus.mode === "direct_provider_development"
    && modelAuthStatus.providerId === selectedProvider;
  const canConnect = modelAuthStatus?.signInAvailable === true;

  // Prop-to-state sync, and the rule is right that it is one. It is kept
  // because the host's answer is the authority here: after a connect or a
  // disconnect the dialog must show the provider and model the host actually
  // holds, not the draft the user typed on the way there.
  //
  // The residual defect is narrow and known: if the host pushes a new model
  // auth status while the user is mid-edit — a projection event rather than
  // their own submit — the draft is discarded under them. Neither documented
  // alternative is better here. Remounting on a `key` would also reset the
  // form notices and pull focus back to the first control; adjusting state
  // during render trades this rule for `react-hooks/set-state-in-render`.
  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    setSelectedProvider(modelSelection.providerId);
    setModelDraft(modelSelection.modelId);
  }, [modelSelection.modelId, modelSelection.providerId]);
  /* eslint-enable react-hooks/set-state-in-effect */

  return (
    <section aria-labelledby="settings-agent-title">
      <header className="settings-pane__header">
        <h3 id="settings-agent-title">Providers</h3>
        <p>Model connections owned by the Forge desktop host.</p>
      </header>
      <div className="provider-summary">
        <span>
          <small>Current model</small>
          <strong>{modelAccessLabel}</strong>
          <em>{modelAccessDetail}</em>
        </span>
      </div>
      <div aria-label="Model providers" className="provider-list">
        {providers.map((provider) => {
          const connected = modelAuthStatus?.status === "development_ready"
            && modelAuthStatus.mode === "direct_provider_development"
            && modelAuthStatus.providerId === provider.id;
          return (
            <label
              className="provider-row provider-row--selectable"
              data-selected={selectedProvider === provider.id || undefined}
              key={provider.id}
            >
              <input
                aria-label={provider.name}
                checked={selectedProvider === provider.id}
                name="model-provider"
                onChange={() => setSelectedProvider(provider.id)}
                type="radio"
                value={provider.id}
              />
              <ProviderIcon className="provider-row__icon" provider={provider.id} />
              <span className="provider-row__copy">
                <strong>{provider.name}</strong>
                <small>{connected ? "Connected for this app session" : provider.setup}</small>
              </span>
              <span className="provider-row__status">
                <em data-connected={connected || undefined}>
                  {connected ? "Connected" : "Needs key"}
                </em>
                {connected && modelAuthStatus.modelId ? (
                  <code>{modelAuthStatus.modelId}</code>
                ) : null}
              </span>
            </label>
          );
        })}
      </div>
      <div className="provider-model-field">
        <label htmlFor="forge-model-id">Model or deployment</label>
        <div>
          <input
            aria-label="Model or deployment"
            autoComplete="off"
            id="forge-model-id"
            onChange={(event) => setModelDraft(event.currentTarget.value)}
            placeholder={selectedProvider === "azure" ? "Azure deployment name" : "OpenAI model ID"}
            spellCheck={false}
            value={modelDraft}
          />
          <Button
            isDisabled={!modelIsValid || modelSelectionBusy}
            onPress={() => void onModelSelectionChange({
              providerId: selectedProvider,
              modelId: modelDraft,
            })}
            size="small"
            variant="primary"
          >
            {modelSelectionBusy ? "Applying…" : "Use model"}
          </Button>
        </div>
        <small>
          {modelSelectionNotice
            ?? "Provider and model are selected here. API credentials are not persisted in frontend settings."}
        </small>
      </div>
      <div className="provider-connection">
        {selectedProviderConnected ? (
          <div className="provider-connection__connected">
            <span>
              <Check aria-hidden="true" size={16} />
              <strong>{selectedProviderName} connected</strong>
              <small>The API key is held only in native memory until Forge exits.</small>
            </span>
            <Button
              aria-label={`Disconnect ${selectedProviderName}`}
              isDisabled={modelSelectionBusy || !modelAuthStatus?.signOutAvailable}
              onPress={() => void onModelDisconnect()}
              size="small"
              variant="secondary"
            >
              <Unplug aria-hidden="true" size={14} /> Disconnect
            </Button>
          </div>
        ) : (
          <form
            aria-label={`Connect ${selectedProviderName}`}
            onSubmit={(event) => {
              event.preventDefault();
              const form = event.currentTarget;
              const credential = String(new FormData(form).get("apiKey") ?? "");
              const keyIsValid = credential.length >= 8
                && credential.length <= 4_096
                // Same denylist as `hostClient/validation.ts`: the control characters are
                // what the guard exists to reject, so a literal control class here is
                // deliberate.
                // eslint-disable-next-line no-control-regex
                && !/[\s\u0000-\u001f\u007f]/u.test(credential);
              if (!keyIsValid) {
                setProviderFormNotice("Enter a valid API key.");
                return;
              }
              if (!canConnect || !modelIsValid || !azureEndpointIsValid) {
                return;
              }
              setProviderFormNotice(null);
              const connection: ModelProviderConnection = {
                providerId: selectedProvider,
                modelId: modelDraft,
                apiKey: credential,
                endpoint: selectedProvider === "azure" ? azureEndpointDraft : null,
                region: selectedProvider === "azure"
                  ? azureRegionDraft.trim() || null
                  : null,
              };
              void onModelConnect(connection).finally(() => {
                setApiKeyDraft("");
                form.reset();
              });
            }}
          >
            <label htmlFor="forge-provider-api-key">
              {selectedProviderName} API key
            </label>
            <div className="provider-connection__secret">
              <KeyRound aria-hidden="true" size={15} />
              <input
                aria-label={`${selectedProviderName} API key`}
                autoComplete="new-password"
                id="forge-provider-api-key"
                name="apiKey"
                onChange={(event) => {
                  setApiKeyDraft(event.currentTarget.value);
                  setProviderFormNotice(null);
                }}
                placeholder="Paste API key"
                spellCheck={false}
                type="password"
                value={apiKeyDraft}
              />
            </div>
            {selectedProvider === "azure" ? (
              <div className="provider-connection__azure">
                <label htmlFor="forge-azure-endpoint">Azure OpenAI v1 endpoint</label>
                <input
                  aria-label="Azure OpenAI v1 endpoint"
                  id="forge-azure-endpoint"
                  onChange={(event) => setAzureEndpointDraft(event.currentTarget.value)}
                  placeholder="https://<resource>.openai.azure.com/openai/v1/"
                  spellCheck={false}
                  value={azureEndpointDraft}
                />
                <small>
                  Use the Azure OpenAI v1 endpoint. Foundry project endpoints ending in
                  {" "}.services.ai.azure.com/api/projects/... are not supported by this
                  local PoC.
                </small>
                <label htmlFor="forge-azure-region">Region (optional)</label>
                <input
                  aria-label="Azure OpenAI region"
                  id="forge-azure-region"
                  onChange={(event) => setAzureRegionDraft(event.currentTarget.value)}
                  placeholder="westeurope"
                  spellCheck={false}
                  value={azureRegionDraft}
                />
              </div>
            ) : null}
            <Button
              aria-label={`Connect ${selectedProviderName}`}
              isDisabled={
                !canConnect
                || !modelIsValid
                || !azureEndpointIsValid
                || modelSelectionBusy
              }
              size="small"
              type="submit"
              variant="primary"
            >
              <PlugZap aria-hidden="true" size={14} /> Connect
            </Button>
            <small>
              {providerFormNotice
                ?? (canConnect
                ? "Sent once to the Rust host, kept in memory, never saved to Forge settings."
                : "Provider connection requires the native Forge desktop host.")}
            </small>
          </form>
        )}
      </div>
      <p className="provider-footnote">
        Provider credentials stay in native memory for this app session and are cleared
        when Forge exits. Agent capability: {agentStatusLabel}.
      </p>
    </section>
  );
}
