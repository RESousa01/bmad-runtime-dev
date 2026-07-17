import {
  HostCommandError,
  PROJECTION_REPLY_SCHEMA,
  type ProjectionEvent,
  type ProjectionEventPayload,
  type ProjectionSnapshot,
} from "./contracts";
import {
  asBoolean,
  asBootMode,
  asContractId,
  asNullableContractId,
  asRecord,
  assertExactKeys,
  asSha256,
  asSingleLineText,
  asUnsignedInteger,
  fail,
} from "./validation";
import { parseLocalHostError } from "./workspaceProtocol";

export function parseProjectionSnapshot(value: unknown): ProjectionSnapshot {
  const snapshot = asRecord(value);
  assertExactKeys(snapshot, [
    "sequence",
    "generatedAt",
    "bootMode",
    "workspaceCount",
    "activeSessionId",
  ]);
  return {
    sequence: asUnsignedInteger(snapshot.sequence),
    generatedAt: asUnsignedInteger(snapshot.generatedAt),
    bootMode: asBootMode(snapshot.bootMode),
    workspaceCount: asUnsignedInteger(snapshot.workspaceCount),
    activeSessionId: asNullableContractId(snapshot.activeSessionId),
  };
}

export function parseProjectionEventPayload(
  value: unknown,
): ProjectionEventPayload {
  const event = asRecord(value);
  assertExactKeys(event, ["type", "projection"]);
  const projection = asRecord(event.projection);
  switch (event.type) {
    case "boot_state_changed":
      assertExactKeys(projection, ["mode"]);
      return {
        type: event.type,
        projection: { mode: asBootMode(projection.mode) },
      };
    case "workspace_changed":
      assertExactKeys(projection, ["workspaceId"]);
      return {
        type: event.type,
        projection: { workspaceId: asContractId(projection.workspaceId) },
      };
    case "bmad.projection_changed":
      assertExactKeys(projection, ["scope"]);
      if (projection.scope !== "library") {
        return fail();
      }
      return { type: event.type, projection: { scope: projection.scope } };
    case "session_changed":
      assertExactKeys(projection, ["sessionId", "state"]);
      return {
        type: event.type,
        projection: {
          sessionId: asContractId(projection.sessionId),
          state: asSingleLineText(projection.state, 64),
        },
      };
    case "approval_required":
      assertExactKeys(projection, ["approvalId", "candidateHash"]);
      return {
        type: event.type,
        projection: {
          approvalId: asContractId(projection.approvalId),
          candidateHash: asSha256(projection.candidateHash),
        },
      };
    case "execution_state_changed":
      assertExactKeys(projection, ["executionId", "state"]);
      return {
        type: event.type,
        projection: {
          executionId: asContractId(projection.executionId),
          state: asSingleLineText(projection.state, 64),
        },
      };
    case "checkpoint_changed":
      assertExactKeys(projection, ["checkpointId", "rollbackAvailable"]);
      return {
        type: event.type,
        projection: {
          checkpointId: asContractId(projection.checkpointId),
          rollbackAvailable: asBoolean(projection.rollbackAvailable),
        },
      };
    case "evidence_changed":
      assertExactKeys(projection, ["streamId"]);
      return {
        type: event.type,
        projection: { streamId: asSingleLineText(projection.streamId, 256) },
      };
    case "connectivity_changed":
      assertExactKeys(projection, ["state"]);
      return {
        type: event.type,
        projection: { state: asSingleLineText(projection.state, 64) },
      };
    case "update_state_changed":
      assertExactKeys(projection, ["state"]);
      return {
        type: event.type,
        projection: { state: asSingleLineText(projection.state, 64) },
      };
    default:
      return fail();
  }
}

export function parseProjectionEvent(value: unknown): ProjectionEvent {
  const event = asRecord(value);
  assertExactKeys(event, ["sequence", "occurredAt", "event"]);
  return {
    sequence: asUnsignedInteger(event.sequence),
    occurredAt: asUnsignedInteger(event.occurredAt),
    event: parseProjectionEventPayload(event.event),
  };
}

export function parseProjectionReply(
  value: unknown,
  rendererSessionId: string,
  expectedStatus: "snapshot" | "events",
): ProjectionSnapshot | ProjectionEvent[] {
  const reply = asRecord(value);
  if (reply.schemaVersion !== PROJECTION_REPLY_SCHEMA) {
    return fail();
  }
  if (reply.status === "error") {
    assertExactKeys(reply, [
      "schemaVersion",
      "rendererSessionId",
      "status",
      "error",
    ]);
    if (
      reply.rendererSessionId !== null &&
      reply.rendererSessionId !== rendererSessionId
    ) {
      return fail();
    }
    throw new HostCommandError(parseLocalHostError(reply.error));
  }
  if (
    reply.status !== expectedStatus ||
    reply.rendererSessionId !== rendererSessionId
  ) {
    return fail();
  }
  if (expectedStatus === "snapshot") {
    assertExactKeys(reply, [
      "schemaVersion",
      "rendererSessionId",
      "status",
      "snapshot",
    ]);
    return parseProjectionSnapshot(reply.snapshot);
  }
  assertExactKeys(reply, [
    "schemaVersion",
    "rendererSessionId",
    "status",
    "events",
  ]);
  if (!Array.isArray(reply.events) || reply.events.length > 512) {
    return fail();
  }
  const events = reply.events.map(parseProjectionEvent);
  for (let index = 1; index < events.length; index += 1) {
    if (events[index]!.sequence <= events[index - 1]!.sequence) {
      return fail();
    }
  }
  return events;
}
