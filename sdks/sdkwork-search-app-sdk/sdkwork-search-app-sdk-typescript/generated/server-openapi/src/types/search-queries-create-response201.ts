import type { PageInfo } from './page-info';
import type { SearchResult } from './search-result';

export interface SearchQueriesCreateResponse201 {
  code: 0;
  data: unknown & { items: SearchResult[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
