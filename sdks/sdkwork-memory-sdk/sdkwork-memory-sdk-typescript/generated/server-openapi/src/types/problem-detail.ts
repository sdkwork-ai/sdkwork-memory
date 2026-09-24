import type { SdkWorkPlatformErrorCode } from './sdk-work-platform-error-code';

export interface ProblemDetail {
  type: string;
  title: string;
  status: number;
  detail?: string;
  instance?: string;
  code: SdkWorkPlatformErrorCode;
  /** Server-generated request correlation id. The same value is returned in the X-SdkWork-Trace-Id response header. */
  traceId: string;
  /** Optional stable localization key such as errors.result.40001. */
  i18nKey?: string;
}
