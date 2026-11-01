import type { PageInfo } from './page-info';
import type { SearchSemanticResult } from './search-semantic-result';

export interface SearchSemanticQueriesCreateResponse201 {
  code: 0;
  data: unknown & { items: SearchSemanticResult[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
