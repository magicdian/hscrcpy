export type VideoCodec = 'h264' | 'jpeg' | 'h265';

export interface CompanionDescriptor {
  shellApiVersion: number;
  protocolMajor: number;
  protocolMinor: number;
  companionName: string;
  nativeCoreVersion: string;
  shellResponsibilities: string[];
  nativeResponsibilities: string[];
  plannedModules: string[];
  supportedSessionChannels: Array<'session' | 'video'>;
  supportedVideoCodecs: Array<'h264' | 'jpeg'>;
}

export interface SessionNegotiationRequest {
  sessionId: string;
  hostSupportedVideoCodecs: VideoCodec[];
  preferredVideoCodecs: VideoCodec[];
  videoMaxWidth: number;
  videoMaxHeight: number;
  videoMaxFps: number;
  bitrateKbps?: number;
  iframeIntervalMs?: number;
  controlEnabled: boolean;
}

export interface HostHelloRequest {
  type: 'host_hello';
  sessionId: string;
  protocolMajor: number;
  protocolMinor: number;
  hostVersion: string;
  requestedFeatures: Array<'video' | 'control'>;
  supportedVideoCodecs: VideoCodec[];
  preferredVideoCodecs: VideoCodec[];
  videoMaxWidth: number;
  videoMaxHeight: number;
  videoMaxFps: number;
  bitrateKbps?: number;
  iframeIntervalMs?: number;
}

export interface JpegSessionConfigRequest {
  sessionId: string;
  selectedVideoCodec: 'jpeg';
  videoMaxWidth: number;
  videoMaxHeight: number;
  videoMaxFps: number;
  controlEnabled: boolean;
}

export interface SessionConfigRequest {
  sessionId: string;
  selectedVideoCodec: VideoCodec;
  videoMaxWidth: number;
  videoMaxHeight: number;
  videoMaxFps: number;
  bitrateKbps?: number;
  iframeIntervalMs?: number;
  controlEnabled: boolean;
}

export interface AuthorizationStateMap {
  videoCapture: 'granted' | 'needs_user_action' | 'denied' | 'unsupported';
  inputInjection: 'granted' | 'needs_user_action' | 'denied' | 'unsupported';
}

export interface VideoCodecDescriptor {
  codec: VideoCodec;
  encoderKind: 'hardware' | 'software';
  maxWidth: number;
  maxHeight: number;
  maxFps: number;
  bitrateControl?: string;
}

export interface DisplayInfo {
  width: number;
  height: number;
  rotation: number;
}

export interface VideoConfig {
  maxWidth: number;
  maxHeight: number;
  maxFps: number;
  bitrateKbps?: number;
  iframeIntervalMs?: number;
}

export interface ControlConfig {
  enabled: boolean;
}

export interface NormalizedPosition {
  x: number;
  y: number;
}

export interface ControlEventRequest {
  type: 'control_event';
  sessionId: string;
  eventType:
    | 'pointer_down'
    | 'pointer_move'
    | 'pointer_up'
    | 'scroll'
    | 'key_down'
    | 'key_up'
    | 'device_action';
  sequence: number;
  positionNorm?: NormalizedPosition;
  pointerId?: number;
  button?: 'primary' | 'secondary' | 'middle';
  scrollDeltaX?: number;
  scrollDeltaY?: number;
  keyCode?: string;
  text?: string;
  deviceAction?: 'back' | 'home' | string;
}

export interface SessionConfigPreview {
  type: 'session_config';
  sessionId: string;
  selectedVideoCodec: VideoCodec;
  video: VideoConfig;
  control: ControlConfig;
}

export interface VideoNegotiationPreview {
  selectedVideoCodec: VideoCodec;
  fallbackAvailable: boolean;
  fallbackVideoCodec: VideoCodec | '';
  fallbackUsed: boolean;
  selectionMode: string;
  selectionReason: string;
  hostSupportedVideoCodecs: VideoCodec[];
  preferredVideoCodecs: VideoCodec[];
  sharedVideoCodecs: VideoCodec[];
  selectedPathModule: string;
}

export interface DeviceHelloPreview {
  type: 'device_hello';
  sessionId: string;
  protocolMajor: number;
  protocolMinor: number;
  companionVersion: string;
  deviceName: string;
  authorization: AuthorizationStateMap;
  availableFeatures: string[];
  availableVideoCodecs: VideoCodecDescriptor[];
  display: DisplayInfo;
}

export interface ChannelBinding {
  name: 'session' | 'video';
  state: string;
  payloadType: string;
  activation: string;
}

export interface ChannelLayout {
  session: ChannelBinding;
  video: ChannelBinding;
}

export interface SessionReadyPreview {
  type: 'session_ready';
  sessionId: string;
  selectedVideoCodec: VideoCodec;
  channelLayout: ChannelLayout;
  display: DisplayInfo;
}

export interface SessionErrorMessage {
  type: 'session_error';
  sessionId: string;
  code: string;
  message: string;
  retryable: boolean;
}

export interface SessionTransportEnvelope {
  channelName: 'session';
  payloadType: 'utf8_json';
  messageType: string;
  payloadJson: string;
}

export interface SessionTransportState {
  sessionId: string;
  protocolMajor: number;
  protocolMinor: number;
  phase: string;
  authorization: AuthorizationStateMap;
  requestedFeatures: string[];
  sharedVideoCodecs: VideoCodec[];
  availableVideoCodecs: VideoCodecDescriptor[];
  selectedVideoCodec: VideoCodec | '';
  controlEnabled: boolean;
  channelLayout: ChannelLayout;
}

export interface VideoTransportState {
  sessionId: string;
  selectedVideoCodec: VideoCodec;
  binding: ChannelBinding;
  display: DisplayInfo;
  selectedPathModule: string;
  pipelineStages: string[];
  active: boolean;
}

export interface SessionTransportOpenResult {
  state: SessionTransportState;
  accepted: boolean;
  deviceHello?: DeviceHelloPreview;
  sessionError?: SessionErrorMessage;
  negotiation: VideoNegotiationPreview;
  suggestedSessionConfig: SessionConfigPreview;
  outboundMessage: SessionTransportEnvelope;
  notes: string[];
}

export interface SessionTransportConfigureResult {
  state: SessionTransportState;
  accepted: boolean;
  sessionReady?: SessionReadyPreview;
  videoTransport?: VideoTransportState;
  sessionError?: SessionErrorMessage;
  outboundMessage: SessionTransportEnvelope;
  pipelineStages: string[];
  notes: string[];
}

export interface StopSessionRequest {
  sessionId: string;
  reason?: string;
}

export interface SessionTransportStopResult {
  state: SessionTransportState;
  accepted: boolean;
  notes: string[];
}

export interface SessionListenerRuntimeStatus {
  running: boolean;
  listenerBound: boolean;
  sessionPort: number;
  state: string;
  lastError: string;
  activeSessionId: string;
}

export interface VideoTransportActivationResult {
  state: VideoTransportState;
  activated: boolean;
  notes: string[];
}

export interface VideoPacketRequest {
  sessionId: string;
  codec: VideoCodec;
  ptsUs: number;
  isKeyframe: boolean;
  payloadLength: number;
}

export interface VideoPacketEnvelope {
  channelName: 'video';
  payloadType: 'binary';
  codec: VideoCodec;
  ptsUs: number;
  isKeyframe: boolean;
  payloadLength: number;
  headerFields: string[];
  delivery: string;
  payloadSemantics: string;
}

export interface VideoPacketTransportResult {
  state: VideoTransportState;
  accepted: boolean;
  packet?: VideoPacketEnvelope;
  notes: string[];
}

export interface VideoUnitPreview {
  codec: VideoCodec;
  unitFields: string[];
  delivery: string;
  keyframeStrategy: string;
  payloadSemantics: string;
}

export interface ControlBaselinePreview {
  channelName: 'session';
  injectorBackend: string;
  executionMode: string;
  supportedEventTypes: string[];
  unsupportedEventTypes: string[];
  supportedDeviceActions: string[];
  pipelineStages: string[];
  notes: string[];
}

export interface ControlHandlingPreview {
  type: 'control_event';
  sessionId: string;
  eventType: ControlEventRequest['eventType'];
  sequence: number;
  route: string;
  accepted: boolean;
  validationResult: 'accepted' | 'rejected';
  injectionStatus: string;
  injectionTarget: string;
  steps: string[];
  notes: string[];
}

export interface SessionNegotiationPreview {
  phase: string;
  sessionId: string;
  deviceHello: DeviceHelloPreview;
  negotiation: VideoNegotiationPreview;
  sessionConfig: SessionConfigPreview;
  sessionReady: SessionReadyPreview;
  controlBaseline: ControlBaselinePreview;
  videoUnit: VideoUnitPreview;
  pipelineStages: string[];
  notes: string[];
}

export const getCompanionDescriptor: () => CompanionDescriptor;
export const previewSessionNegotiation: (request: SessionNegotiationRequest) => SessionNegotiationPreview;
export const previewJpegSessionPath: (request: JpegSessionConfigRequest) => SessionNegotiationPreview;
export const previewControlHandling: (request: ControlEventRequest) => ControlHandlingPreview;
export const openSessionTransport: (request: HostHelloRequest) => SessionTransportOpenResult;
export const configureSessionTransport: (
  state: SessionTransportState,
  request: SessionConfigRequest
) => SessionTransportConfigureResult;
export const startSessionListenerRuntime: (sessionPort?: number) => SessionListenerRuntimeStatus;
export const stopSessionListenerRuntime: () => SessionListenerRuntimeStatus;
export const getSessionListenerRuntimeStatus: () => SessionListenerRuntimeStatus;
export const dispatchControlEvent: (
  state: SessionTransportState,
  request: ControlEventRequest
) => ControlHandlingPreview;
export const activateVideoTransport: (state: VideoTransportState) => VideoTransportActivationResult;
export const prepareVideoTransportPacket: (
  state: VideoTransportState,
  request: VideoPacketRequest
) => VideoPacketTransportResult;
export const stopSessionTransport: (
  state: SessionTransportState,
  request: StopSessionRequest
) => SessionTransportStopResult;

declare const nativeBridge: {
  getCompanionDescriptor: typeof getCompanionDescriptor;
  previewSessionNegotiation: typeof previewSessionNegotiation;
  previewJpegSessionPath: typeof previewJpegSessionPath;
  previewControlHandling: typeof previewControlHandling;
  openSessionTransport: typeof openSessionTransport;
  configureSessionTransport: typeof configureSessionTransport;
  startSessionListenerRuntime: typeof startSessionListenerRuntime;
  stopSessionListenerRuntime: typeof stopSessionListenerRuntime;
  getSessionListenerRuntimeStatus: typeof getSessionListenerRuntimeStatus;
  dispatchControlEvent: typeof dispatchControlEvent;
  activateVideoTransport: typeof activateVideoTransport;
  prepareVideoTransportPacket: typeof prepareVideoTransportPacket;
  stopSessionTransport: typeof stopSessionTransport;
};

export default nativeBridge;
