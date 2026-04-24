const H264_SESSION_PREVIEW = {
  phase: 'video_negotiation',
  sessionId: 'shell-preview',
  deviceHello: {
    type: 'device_hello',
    sessionId: 'shell-preview',
    protocolMajor: 1,
    protocolMinor: 0,
    companionVersion: '0.4.0-h264-mainline',
    deviceName: 'HarmonyOS Companion Preview Device',
    authorization: {
      videoCapture: 'granted',
      inputInjection: 'granted',
    },
    availableFeatures: ['video', 'control'],
    availableVideoCodecs: [
      {
        codec: 'h264',
        encoderKind: 'hardware',
        maxWidth: 1920,
        maxHeight: 1080,
        maxFps: 120,
        bitrateControl: 'vbr',
      },
      {
        codec: 'jpeg',
        encoderKind: 'software',
        maxWidth: 1920,
        maxHeight: 1080,
        maxFps: 30,
      },
    ],
    display: {
      width: 1920,
      height: 1080,
      rotation: 0,
    },
  },
  negotiation: {
    selectedVideoCodec: 'h264',
    fallbackAvailable: true,
    fallbackVideoCodec: 'jpeg',
    fallbackUsed: false,
    selectionMode: 'host_preference_order',
    selectionReason: 'preferred_common_codec',
    hostSupportedVideoCodecs: ['h264', 'jpeg'],
    preferredVideoCodecs: ['h264', 'jpeg'],
    sharedVideoCodecs: ['h264', 'jpeg'],
    selectedPathModule: 'video/h264_video_path',
  },
  sessionConfig: {
    type: 'session_config',
    sessionId: 'shell-preview',
    selectedVideoCodec: 'h264',
    video: {
      maxWidth: 1280,
      maxHeight: 720,
      maxFps: 30,
      bitrateKbps: 8000,
      iframeIntervalMs: 2000,
    },
    control: {
      enabled: true,
    },
  },
  sessionReady: {
    type: 'session_ready',
    sessionId: 'shell-preview',
    selectedVideoCodec: 'h264',
    channelLayout: {
      session: {
        name: 'session',
        state: 'ready',
        payloadType: 'utf8_json',
        activation: 'opened before session_config and kept for control plus teardown',
      },
      video: {
        name: 'video',
        state: 'pending_open',
        payloadType: 'binary',
        activation: 'opens after session_ready for H.264 access-unit delivery',
      },
    },
    display: {
      width: 1280,
      height: 720,
      rotation: 0,
    },
  },
  controlBaseline: {
    channelName: 'session',
    injectorBackend: 'harmony_input_injector_scaffold',
    executionMode: 'validated_scaffold',
    supportedEventTypes: ['pointer_down', 'pointer_move', 'pointer_up', 'device_action'],
    unsupportedEventTypes: ['scroll', 'key_down', 'key_up'],
    supportedDeviceActions: ['back', 'home'],
    pipelineStages: [
      'transport/session_channel',
      'control/control_message_handler',
      'control/input_injector',
    ],
    notes: [
      'control_event stays on the session channel and never enters the video path.',
      'Pointer and device-action events are validated and routed through a native injection scaffold.',
      'Scroll and keyboard events remain explicitly unsupported until HarmonyOS bindings land.',
    ],
  },
  videoUnit: {
    codec: 'h264',
    unitFields: ['codec', 'pts_us', 'is_keyframe', 'payload_length'],
    delivery: 'one H.264 access unit per binary payload unit on the video channel',
    keyframeStrategy: 'keyframes follow host-selected iframe_interval_ms when provided, otherwise the platform default cadence',
    payloadSemantics: 'payload bytes stay provisional, but every unit still carries the host-device contract fields',
  },
  pipelineStages: [
    'capture/display_capture',
    'video/h264_video_path',
    'transport/session_channel',
    'transport/video_channel',
  ],
  notes: [
    'device_hello is shaped in native code and advertises both the H.264 mainline and JPEG fallback when shared with the host.',
    'control.enabled continues to route through the explicit device-side control baseline.',
    'control_event validation and injection planning stay in native code; ArkTS only renders the preview state.',
    'The video channel stays binary-only and activates after session_ready.',
    'H.264 is selected as the preferred shared codec for the current session preview.',
    'JPEG remains advertised as an explicit fallback path if a later session cannot use H.264.',
  ],
};

const JPEG_SESSION_PREVIEW = {
  phase: 'video_negotiation',
  sessionId: 'shell-preview',
  deviceHello: {
    type: 'device_hello',
    sessionId: 'shell-preview',
    protocolMajor: 1,
    protocolMinor: 0,
    companionVersion: '0.4.0-h264-mainline',
    deviceName: 'HarmonyOS Companion Preview Device',
    authorization: {
      videoCapture: 'granted',
      inputInjection: 'granted',
    },
    availableFeatures: ['video', 'control'],
    availableVideoCodecs: [
      {
        codec: 'h264',
        encoderKind: 'hardware',
        maxWidth: 1920,
        maxHeight: 1080,
        maxFps: 120,
        bitrateControl: 'vbr',
      },
      {
        codec: 'jpeg',
        encoderKind: 'software',
        maxWidth: 1920,
        maxHeight: 1080,
        maxFps: 30,
      },
    ],
    display: {
      width: 1920,
      height: 1080,
      rotation: 0,
    },
  },
  negotiation: {
    selectedVideoCodec: 'jpeg',
    fallbackAvailable: true,
    fallbackVideoCodec: 'jpeg',
    fallbackUsed: false,
    selectionMode: 'host_preference_order',
    selectionReason: 'preferred_common_codec',
    hostSupportedVideoCodecs: ['jpeg'],
    preferredVideoCodecs: ['jpeg'],
    sharedVideoCodecs: ['jpeg'],
    selectedPathModule: 'video/jpeg_video_path',
  },
  sessionConfig: {
    type: 'session_config',
    sessionId: 'shell-preview',
    selectedVideoCodec: 'jpeg',
    video: {
      maxWidth: 1280,
      maxHeight: 720,
      maxFps: 15,
    },
    control: {
      enabled: true,
    },
  },
  sessionReady: {
    type: 'session_ready',
    sessionId: 'shell-preview',
    selectedVideoCodec: 'jpeg',
    channelLayout: {
      session: {
        name: 'session',
        state: 'ready',
        payloadType: 'utf8_json',
        activation: 'opened before session_config and kept for control plus teardown',
      },
      video: {
        name: 'video',
        state: 'pending_open',
        payloadType: 'binary',
        activation: 'opens after session_ready for JPEG frame delivery',
      },
    },
    display: {
      width: 1280,
      height: 720,
      rotation: 0,
    },
  },
  videoUnit: {
    codec: 'jpeg',
    unitFields: ['codec', 'pts_us', 'is_keyframe', 'payload_length'],
    delivery: 'one JPEG frame per binary payload unit on the video channel',
    keyframeStrategy: 'every JPEG frame is independently decodable and treated as a keyframe',
    payloadSemantics: 'header semantics are fixed by the host-device contract even though byte packing stays provisional',
  },
  pipelineStages: [
    'capture/display_capture',
    'video/jpeg_video_path',
    'transport/session_channel',
    'transport/video_channel',
  ],
  notes: [
    'device_hello is shaped in native code and advertises both the H.264 mainline and JPEG fallback when shared with the host.',
    'control.enabled continues to route through the explicit device-side control baseline.',
    'control_event validation and injection planning stay in native code; ArkTS only renders the preview state.',
    'The video channel stays binary-only and activates after session_ready.',
    'JPEG is selected for the current session preview.',
  ],
};

const NativeMock: Record<string, Object> = {
  'getCompanionDescriptor': () => {
    return {
      shellApiVersion: 1,
      protocolMajor: 1,
      protocolMinor: 0,
      companionName: 'hscrcpy Harmony Companion',
      nativeCoreVersion: '0.4.0-h264-mainline',
      shellResponsibilities: [
        'UIAbility lifecycle and window loading',
        'permission and configuration surfaces',
        'typed bridge calls into the native core',
      ],
      nativeResponsibilities: [
        'session ownership, startup validation, and host-order video negotiation preview',
        'H.264 mainline capability advertisement with explicit JPEG fallback shaping',
        'control_event validation plus input injection scaffolding behind a thin N-API bridge',
        'capture, video-path, and transport boundaries behind a thin N-API bridge',
      ],
      plannedModules: [
        'bridge',
        'capture/display_capture',
        'control/control_message_handler',
        'control/input_injector',
        'video/h264_video_path',
        'video/jpeg_video_path',
        'video/video_path_utils',
        'transport/session_channel',
        'transport/video_channel',
      ],
      supportedSessionChannels: ['session', 'video'],
      supportedVideoCodecs: ['h264', 'jpeg'],
    };
  },
  'previewSessionNegotiation': () => {
    return H264_SESSION_PREVIEW;
  },
  'previewJpegSessionPath': () => {
    return JPEG_SESSION_PREVIEW;
  },
  'previewControlHandling': () => {
    return {
      type: 'control_event',
      sessionId: 'shell-preview',
      eventType: 'pointer_down',
      sequence: 1,
      route: 'session/control_event -> control/control_message_handler -> control/input_injector',
      accepted: true,
      validationResult: 'accepted',
      injectionStatus: 'pending_platform_binding',
      injectionTarget: 'pointer_surface',
      steps: [
        'read control_event envelope',
        'validate message against the host-device MVP contract',
        'resolve input injector backend',
        'check baseline support for the requested control_event',
        'map normalized coordinates onto the active device display',
        'hand pointer intent to the HarmonyOS injector scaffold',
      ],
      notes: [
        'The baseline accepts pointer events and keeps the coordinate mapping on-device.',
        'Replacing the scaffold with concrete HarmonyOS injection APIs is a follow-up task.',
      ],
    };
  },
};

export default NativeMock;
