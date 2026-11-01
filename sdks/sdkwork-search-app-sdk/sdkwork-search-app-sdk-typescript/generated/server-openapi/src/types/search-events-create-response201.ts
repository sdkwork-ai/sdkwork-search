import type { SearchUserEventResponse } from './search-user-event-response';

export interface SearchEventsCreateResponse201 {
  code: 0;
  data: unknown & { item: SearchUserEventResponse; };
  /** Server-owned request correlation id. */
  traceId: string;
}
