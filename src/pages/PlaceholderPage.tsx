import { Hammer } from 'lucide-react'

import { Card, Empty, Page } from '@/components/ui'

/**
 * 아직 만들지 않은 화면의 자리표시.
 * 어느 Phase에서 채워지는지 적어 두어 사용자가 기대치를 알 수 있게 한다.
 */
export function PlaceholderPage({
  title,
  phase,
  description,
}: {
  title: string
  phase: number
  description: string
}) {
  return (
    <Page title={title}>
      <Card>
        <Empty
          icon={Hammer}
          title={`${title} 화면은 Phase ${phase}에서 만들어집니다`}
          description={description}
        />
      </Card>
    </Page>
  )
}
