import type { PageInfo } from './page-info';
import type { SearchRecommendationItem } from './search-recommendation-item';

export interface SearchRecommendationsCreateResponse201 {
  code: 0;
  data: unknown & { items: SearchRecommendationItem[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
