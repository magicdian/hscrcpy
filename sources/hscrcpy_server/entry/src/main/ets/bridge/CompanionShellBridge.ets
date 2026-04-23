import nativeBridge, { VideoCodecDescriptor, VideoConfig } from 'libentry.so';

export interface CompanionShellSection {
  title: string;
  lines: string[];
}

export interface CompanionShellModel {
  title: string;
  subtitle: string;
  badge: string;
  boundarySummary: string;
  sections: CompanionShellSection[];
}

const PREVIEW_SESSION_ID = 'shell-preview';

function describeCodec(codec: VideoCodecDescriptor): string {
  const bitrateControl = codec.bitrateControl ? `:${codec.bitrateControl}` : '';
  return `${codec.codec}:${codec.encoderKind}:${codec.maxWidth}x${codec.maxHeight}@${codec.maxFps}${bitrateControl}`;
}

function describeSessionVideo(video: VideoConfig): string {
  const bitrate = video.bitrateKbps ? `; bitrate=${video.bitrateKbps}` : '';
  const iframeInterval = video.iframeIntervalMs ? `; iframe_interval_ms=${video.iframeIntervalMs}` : '';
  return `${video.maxWidth}x${video.maxHeight}@${video.maxFps}${bitrate}${iframeInterval}`;
}

export function loadCompanionShellModel(): CompanionShellModel {
  const descriptor = nativeBridge.getCompanionDescriptor();
  const preview = nativeBridge.previewSessionNegotiation({
    sessionId: PREVIEW_SESSION_ID,
    hostSupportedVideoCodecs: ['h264', 'jpeg'],
    preferredVideoCodecs: ['h264', 'jpeg'],
    videoMaxWidth: 1280,
    videoMaxHeight: 720,
    videoMaxFps: 30,
    bitrateKbps: 8000,
    iframeIntervalMs: 2000,
    controlEnabled: true,
  });
  const controlPreview = nativeBridge.previewControlHandling({
    type: 'control_event',
    sessionId: PREVIEW_SESSION_ID,
    eventType: 'pointer_down',
    sequence: 1,
    positionNorm: {
      x: 0.25,
      y: 0.4,
    },
    pointerId: 0,
    button: 'primary',
  });
  const deviceCodecs = preview.deviceHello.availableVideoCodecs.map(describeCodec).join(', ');

  return {
    title: descriptor.companionName,
    subtitle: `Shell API v${descriptor.shellApiVersion} | Protocol ${descriptor.protocolMajor}.${descriptor.protocolMinor} | Native core ${descriptor.nativeCoreVersion}`,
    badge: 'H264 MAINLINE + JPEG FALLBACK',
    boundarySummary: 'ArkTS owns lifecycle and rendering. Native owns device codec advertisement, host-order negotiation preview, startup shaping, and the active video path.',
    sections: [
      {
        title: 'Shell responsibilities',
        lines: descriptor.shellResponsibilities,
      },
      {
        title: 'Native responsibilities',
        lines: descriptor.nativeResponsibilities,
      },
      {
        title: 'Native session modules',
        lines: descriptor.plannedModules,
      },
      {
        title: 'Device hello preview',
        lines: [
          `type=${preview.deviceHello.type}; device=${preview.deviceHello.deviceName}`,
          `authorization.video_capture=${preview.deviceHello.authorization.videoCapture}`,
          `authorization.input_injection=${preview.deviceHello.authorization.inputInjection}`,
          `available_features=${preview.deviceHello.availableFeatures.join(', ')}`,
          `available_video_codecs=${deviceCodecs}`,
          `display=${preview.deviceHello.display.width}x${preview.deviceHello.display.height}@rotation${preview.deviceHello.display.rotation}`,
        ],
      },
      {
        title: 'Negotiated video path',
        lines: [
          `selected=${preview.negotiation.selectedVideoCodec}; fallback_available=${preview.negotiation.fallbackAvailable}; fallback_used=${preview.negotiation.fallbackUsed}`,
          `fallback_codec=${preview.negotiation.fallbackVideoCodec || 'none'}; mode=${preview.negotiation.selectionMode}`,
          `reason=${preview.negotiation.selectionReason}`,
          `host_supported=${preview.negotiation.hostSupportedVideoCodecs.join(', ')}`,
          `preferred=${preview.negotiation.preferredVideoCodecs.join(', ')}`,
          `shared=${preview.negotiation.sharedVideoCodecs.join(', ')}`,
          `selected_path_module=${preview.negotiation.selectedPathModule}`,
        ],
      },
      {
        title: 'Session preview',
        lines: [
          `phase=${preview.phase}; codec=${preview.sessionConfig.selectedVideoCodec}; control=${preview.sessionConfig.control.enabled}`,
          `session_config.video=${describeSessionVideo(preview.sessionConfig.video)}`,
          `session_ready.display=${preview.sessionReady.display.width}x${preview.sessionReady.display.height}@rotation${preview.sessionReady.display.rotation}`,
          `channel_layout.session=${preview.sessionReady.channelLayout.session.payloadType}:${preview.sessionReady.channelLayout.session.state}`,
          `channel_layout.video=${preview.sessionReady.channelLayout.video.payloadType}:${preview.sessionReady.channelLayout.video.state}`,
          ...preview.pipelineStages.map((stage) => `pipeline=${stage}`),
        ],
      },
      {
        title: 'Control baseline',
        lines: [
          `channel=${preview.controlBaseline.channelName}; backend=${preview.controlBaseline.injectorBackend}`,
          `execution_mode=${preview.controlBaseline.executionMode}`,
          `supported_events=${preview.controlBaseline.supportedEventTypes.join(', ')}`,
          `unsupported_events=${preview.controlBaseline.unsupportedEventTypes.join(', ')}`,
          `device_actions=${preview.controlBaseline.supportedDeviceActions.join(', ')}`,
          ...preview.controlBaseline.pipelineStages.map((stage) => `pipeline=${stage}`),
          ...preview.controlBaseline.notes,
        ],
      },
      {
        title: 'Control event handling',
        lines: [
          `type=${controlPreview.type}; event=${controlPreview.eventType}; sequence=${controlPreview.sequence}`,
          `accepted=${controlPreview.accepted}; validation=${controlPreview.validationResult}`,
          `injection_status=${controlPreview.injectionStatus}; target=${controlPreview.injectionTarget}`,
          `route=${controlPreview.route}`,
          ...controlPreview.steps.map((step) => `step=${step}`),
          ...controlPreview.notes,
        ],
      },
      {
        title: 'Video unit semantics',
        lines: [
          `codec=${preview.videoUnit.codec}; delivery=${preview.videoUnit.delivery}`,
          `fields=${preview.videoUnit.unitFields.join(', ')}`,
          `keyframe_strategy=${preview.videoUnit.keyframeStrategy}`,
          preview.videoUnit.payloadSemantics,
          ...descriptor.supportedSessionChannels.map((channel) => `channel=${channel}`),
          ...descriptor.supportedVideoCodecs.map((codec) => `advertised_codec=${codec}`),
          ...preview.notes,
        ],
      },
    ],
  };
}
