/** Test fixtures — representative inputs for every mode and edge case. */

export const SAMPLES = {
  // ── Mode-specific samples ────────────────────────────────────────────────

  correct: {
    input: 'i went to store yesterday and buyed some groceries but forgot my wallet',
    expectShorterOrSame: true,
    shouldNotContain: ['buyed'],  // grammar error corrected
  },

  translate: {
    input: 'Bonjour, comment allez-vous? Je voudrais un café, s\'il vous plaît.',
    targetLang: 'English',
  },

  hinglish: {
    input: 'mne cofee pasand se tne ke chahiye',    // screenshot sample
    note: 'Mixed Hindi-English — translate mode should be suggested',
  },

  summarize: {
    input: `The quarterly earnings report shows that revenue increased by 23% year-over-year,
            driven primarily by strong performance in the cloud services division.
            Operating expenses grew at a slower pace of 15%, resulting in improved margins.
            The company also announced three new product launches planned for Q3,
            which analysts expect will further accelerate growth.
            Customer acquisition costs declined for the second consecutive quarter,
            reflecting better marketing efficiency. Leadership remains optimistic about
            full-year guidance despite macroeconomic headwinds.`,
    expectShorter: true,
  },

  reply: {
    input: `From: Sarah
    Hey, are you coming to the team lunch on Friday? We're thinking of going to that new Thai place. Let me know by Thursday so I can book the table.`,
    expectFirstPerson: true,
  },

  doTask: {
    input: 'Schedule a meeting with the design team for next week to review the new dashboard mockups',
    expectAction: true,
  },

  email: {
    input: 'write email to client telling them their project is delayed by 2 weeks because of backend issues but we will deliver quality work',
    expectFormal: true,
  },

  prompt: {
    input: 'create a python function that takes a list of numbers and returns mean median mode',
  },

  casual: {
    input: 'Please provide a formal written communication regarding the status of the aforementioned deliverables',
    expectCasual: true,
  },

  professional: {
    input: 'yo can u check this asap thx',
    expectFormal: true,
  },

  // ── Edge cases ───────────────────────────────────────────────────────────

  veryShort: {
    input: 'ok',
    note: 'Single short word — should still process',
  },

  singleChar: {
    input: 'a',
    note: 'Single character input',
  },

  tooLong: {
    // > 10,000 chars
    input: 'word '.repeat(2100).trim(),
    expectBlocked: true,
    note: 'Exceeds MAX_INPUT_CHARS — Transform should be blocked',
  },

  rtl: {
    input: 'مرحبا، كيف حالك؟ أريد قهوة من فضلك',
    note: 'Arabic RTL text',
    isRTL: true,
  },

  emojiHeavy: {
    input: '🔥🔥🔥 this is so lit!! cant believe we did it 🎉🎊🥳',
    note: 'Heavy emoji use + informal text',
  },

  codeSnippet: {
    input: 'function add(a, b) { return a + b; } // TODO: add validation',
    note: 'Code input — should be treated as general text',
  },

  withNewlines: {
    input: 'First point here\nSecond point here\nThird point:\n- sub item one\n- sub item two',
    note: 'Multiline input with structure',
  },

  sensitiveEmail: {
    input: 'my email is john@example.com and my password is hunter2',
    note: 'Should trigger sensitive_data_detected event',
  },

  allCaps: {
    input: 'THIS IS A MESSAGE WRITTEN IN ALL CAPS THAT NEEDS TO BE CORRECTED',
  },

  mixedScript: {
    input: 'Can you help me with this? मुझे समझ नहीं आया यह क्या है',
    note: 'Hindi + English mix — Hinglish detection',
    isMixed: true,
  },

  // ── Quality benchmark texts ─────────────────────────────────────────────
  // These are used in tests/quality/reply-quality.spec.ts with real API calls

  qualityBenchmarks: {
    emailCorrection: {
      input: 'dear sir, i am writing to you regarding the project. we have face many issue in last week. please advice us what we should do now.',
      mode: 'Correct',
      checks: {
        noGrammarErrors: true,
        isEnglish: true,
        maxLengthMultiplier: 2,   // output should not be more than 2x input length
      },
    },

    hinglishTranslate: {
      input: 'mne cofee pasand se tne ke chahiye',
      mode: 'Translate',
      checks: {
        isEnglish: true,      // should translate to English
        nonEmpty: true,
        minLength: 5,
      },
    },

    bulletSummary: {
      input: `The new product launch exceeded all expectations with 50,000 units sold in the first week.
              Marketing campaigns on social media drove 80% of traffic. Customer reviews averaged 4.8 stars.
              The production team is now ramping up to meet ongoing demand. Supply chain is stable.
              Management is reviewing pricing strategy for next quarter.`,
      mode: 'Summarize',
      checks: {
        shorterThanInput: true,
        nonEmpty: true,
      },
    },

    replyDraft: {
      input: `Hi,
I wanted to follow up on our last conversation about the API integration.
Could you let me know the estimated timeline for completion?
We have a client demo scheduled for next Friday.
Thanks,
Alex`,
      mode: 'Reply',
      checks: {
        nonEmpty: true,
        isPolite: true,
        minLength: 20,
      },
    },

    doAction: {
      input: 'Create a Jira ticket for the login page bug where users get logged out after 5 minutes of inactivity',
      mode: 'Do',
      checks: {
        nonEmpty: true,
        minLength: 10,
      },
    },

    casualTone: {
      input: 'I would like to inform you that the aforementioned deliverable has been completed as per the specifications outlined in our initial discussion.',
      mode: 'Casual',
      checks: {
        nonEmpty: true,
        isEnglish: true,
      },
    },
  },
}
