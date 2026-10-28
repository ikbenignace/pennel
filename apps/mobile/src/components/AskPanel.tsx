import React, { useState } from 'react';
import { useRouter } from 'expo-router';
import { View } from 'react-native';
import type { ChatTurn, MeetingAnswer } from '../domain';
import { useRuntime } from '../runtime';
import { meetingAnswer } from '../normalize';
import { rememberSource } from '../source-navigation';
import { Body, Button, Field, Row, Section } from './ui';

export function AskPanel({ meetingId }: { meetingId?: string }) {
  const { run, busy, t, theme } = useRuntime();
  const [question, setQuestion] = useState('');
  const [turns, setTurns] = useState<{ question: string; answer: MeetingAnswer }[]>([]);
  const router = useRouter();
  const key = `ask-${meetingId ?? 'all'}`;
  const ask = async () => {
    const text = question.trim();
    if (!text || busy.has(key)) return;
    const history: ChatTurn[] = turns.map((turn) => ({ question: turn.question, answer: turn.answer.text }));
    const result = await run<MeetingAnswer>(key, 'meetings.ask', { question: text, history, meeting_id: meetingId ?? null });
    if (result) { setTurns((previous) => [...previous, { question: text, answer: meetingAnswer(result) }]); setQuestion(''); }
  };
  return <>
    {turns.map((turn, index) => <Section key={index} title={turn.question}><Body selectable>{turn.answer.text}</Body>{turn.answer.citations.map((citation) => <Row key={citation.number} title={`${citation.number}. ${citation.passage.title}`} subtitle={t(`meetings.source_${citation.passage.kind}`)} onPress={() => router.push({ pathname: '/meeting/[id]', params: { id: citation.passage.meeting_id, source: rememberSource(citation.passage) } })} />)}</Section>)}
    <View style={{ gap: theme.space[3] }}><Field label={t('meetings.your_question')} placeholder={t(meetingId ? 'meetings.ask_placeholder_this' : 'meetings.ask_placeholder_all')} value={question} onChangeText={setQuestion} multiline style={{ minHeight: 88 }} returnKeyType="default" /><Button title={t(busy.has(key) ? 'meetings.writing_answer' : 'meetings.ask')} icon="arrow-up" busy={busy.has(key)} disabled={!question.trim()} onPress={() => void ask()} /></View>
  </>;
}
