import type { PageInfo } from './page-info';
import type { SearchPromotionItem } from './search-promotion-item';

export interface SearchPromotionsCreateResponse201 {
  code: 0;
  data: unknown & { items: SearchPromotionItem[]; pageInfo: PageInfo; };
  /** Server-owned request correlation id. */
  traceId: string;
}
