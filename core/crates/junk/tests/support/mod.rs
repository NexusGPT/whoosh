//! Fixture corpora, written for these tests: every sender is on a reserved example domain, every line is invented,
//! no real mail is in this crate. 36 junk lines and 36 not-junk lines; the junk lines share a junk vocabulary
//! (free, winner, prize, cash, click, offer, casino, pills, loan, bitcoin...) and the not-junk lines a working-life
//! vocabulary (meeting, agenda, budget, lunch, roadmap, deadline...), with ordinary words in both.
#![allow(dead_code)]
use junk::{Context, Message};

pub fn msg(name: Option<&str>, email: &str, subject: &str, text: &str) -> Message {
    Message {
        from_name: name.map(str::to_string),
        from_email: email.to_string(),
        reply_to: Vec::new(),
        subject: subject.to_string(),
        text: text.to_string(),
        html: None,
        authentication_results: Vec::new(),
    }
}

pub fn ctx() -> Context {
    Context::default()
}

/// (sender, subject, text)
const SPAM: [(&str, &str, &str); 36] = [
    ("deals@promo-deals.example", "WIN a free cruise today", "Congratulations winner, claim your free cruise prize now. Click here, limited offer, cash bonus included."),
    ("deals@promo-deals.example", "Free cash bonus waiting", "Your free cash bonus is waiting. Click to claim the prize, this offer is limited and guaranteed."),
    ("deals@promo-deals.example", "Cheap pills, big discount", "Cheap pills from our online pharmacy. Discount offer, order now, no prescription, guaranteed delivery."),
    ("lucky@win-now.example", "You are a lottery winner", "Lottery winner! Claim your million prize today. Wire transfer fee only, urgent, click the link."),
    ("lucky@win-now.example", "Casino bonus: free spins", "Join the casino today for a free bonus and free spins. Win cash, click now, limited time offer."),
    ("lucky@win-now.example", "Urgent: claim your prize", "Urgent notice, you are the winner of a cash prize. Click to claim, offer is limited."),
    ("pharma@cheap-meds.example", "Miracle weight loss pills", "Miracle weight loss pills, guaranteed results. Cheap pharmacy discount, order your pills now."),
    ("pharma@cheap-meds.example", "Pharmacy discount offer", "Online pharmacy with cheap pills and a big discount. Limited offer, click to order, free shipping."),
    ("pharma@cheap-meds.example", "Lose weight fast", "Lose weight fast with miracle pills. Guaranteed weight loss or free cash back, click now."),
    ("gold@crypto-gold.example", "Bitcoin investment, guaranteed", "Bitcoin investment with guaranteed returns. Crypto profits, turn a small investment into a million, click now."),
    ("gold@crypto-gold.example", "Crypto cash bonus", "Free crypto bonus for new investors. Claim your bitcoin cash prize, limited offer, urgent."),
    ("gold@crypto-gold.example", "Double your bitcoin", "Double your bitcoin today. Crypto investment, guaranteed profit, wire transfer, click here."),
    ("loans@loans-fast.example", "Fast loan approval", "Fast loan approval, bad credit welcome. Cash in minutes, guaranteed mortgage and credit offer, click now."),
    ("loans@loans-fast.example", "Credit offer: free cash", "Free cash credit offer. Get a loan today, guaranteed approval, limited time, click to claim."),
    ("loans@loans-fast.example", "Urgent mortgage discount", "Urgent mortgage discount, lowest rate guaranteed. Loan offer limited, claim your cash bonus."),
    ("prince@inherit.example", "Inheritance of a million", "I am writing about an inheritance of a million dollars. Wire transfer needed, urgent and confidential, claim your prize."),
    ("prince@inherit.example", "Confidential inheritance transfer", "Confidential transfer of an inheritance. Cash million waiting, send your bank details, urgent."),
    ("prince@inherit.example", "Urgent wire transfer", "Urgent wire transfer of a million, guaranteed share of the inheritance. Click to claim."),
    ("promo@promo-deals.example", "Free gift, click now", "Free gift for you, click now. Limited offer, claim your bonus prize before it ends."),
    ("promo@promo-deals.example", "Discount: cheap watches", "Cheap watches with a huge discount. Limited offer, free shipping, click to order."),
    ("promo@promo-deals.example", "Winner! Free vacation", "Winner of a free vacation cruise. Claim your prize, limited offer, cash bonus, click here."),
    ("bets@win-now.example", "Casino night, free cash", "Casino night with free cash bonus. Win a million, limited offer, click to join."),
    ("bets@win-now.example", "Lottery: last chance", "Last chance lottery prize. You are a winner, claim your cash, urgent, click now."),
    ("bets@win-now.example", "Free casino spins", "Free casino spins and a cash bonus. Winner every day, limited offer, click to play."),
    ("health@cheap-meds.example", "Pills without prescription", "Cheap pills without prescription. Pharmacy discount, guaranteed delivery, click to order."),
    ("health@cheap-meds.example", "Weight loss miracle", "The miracle of weight loss in a pill. Guaranteed, cheap, limited offer, order now."),
    ("health@cheap-meds.example", "Free pills sample", "Free pills sample from our pharmacy. Discount on your order, limited offer, click here."),
    ("coin@crypto-gold.example", "Crypto giveaway", "Crypto giveaway, free bitcoin for the first investors. Claim your prize, limited offer, click now."),
    ("coin@crypto-gold.example", "Investment opportunity", "Investment opportunity: guaranteed bitcoin returns, a million in profit. Urgent, click to invest."),
    ("coin@crypto-gold.example", "Bitcoin cash prize", "Bitcoin cash prize for you. Free crypto, claim today, limited offer, urgent."),
    ("cash@loans-fast.example", "Need cash? Loan today", "Need cash? Loan today with guaranteed approval. Credit offer, click to claim your bonus."),
    ("cash@loans-fast.example", "Free credit check offer", "Free credit check offer and a cash loan. Guaranteed, limited, click now."),
    ("cash@loans-fast.example", "Mortgage offer: claim now", "Mortgage offer, claim now. Guaranteed low rate, cash bonus, limited time, urgent."),
    ("lucky@win-now.example", "Congratulations winner", "Congratulations winner, a free prize and a cash bonus are waiting. Click to claim, limited offer."),
    ("deals@promo-deals.example", "Cheap deals, free bonus", "Cheap deals and a free bonus, limited offer. Discount on everything, click now, guaranteed."),
    ("prince@inherit.example", "Million dollar transfer", "A million dollar transfer is waiting for you. Inheritance, urgent, wire, claim your cash prize."),
];

/// (sender, subject, text)
const HAM: [(&str, &str, &str); 36] = [
    ("colleague@work.example", "Meeting notes for Thursday", "Attached are the notes from the planning meeting. Please review the budget table before we meet again."),
    ("colleague@work.example", "Agenda for tomorrow", "Here is the agenda for tomorrow's team meeting: roadmap, budget review and the deadline for the report."),
    ("colleague@work.example", "Draft of the quarterly report", "I put the draft of the quarterly report in the shared folder. Feedback welcome before the deadline."),
    ("team@work.example", "Project schedule update", "The project schedule moves by a week. Roadmap and minutes of the meeting are in the folder, thanks."),
    ("team@work.example", "Review of the presentation", "Could you review the presentation for the quarterly meeting? Send your feedback by tomorrow, regards."),
    ("team@work.example", "Minutes of the team meeting", "Minutes of the team meeting are attached. The next deadline is for the roadmap draft, thanks for the notes."),
    ("friend@mail.example", "Lunch tomorrow?", "Are you free for lunch tomorrow? I know a nice place near the office with a good garden, regards."),
    ("friend@mail.example", "Photos from the weekend", "Here are the photos from the weekend. The garden looked great, thanks for dinner, see you soon."),
    ("friend@mail.example", "Dinner on Saturday", "Dinner on Saturday at my place? I will cook the recipe from last weekend, bring photos of the trip."),
    ("family@home.example", "Birthday plans", "Birthday plans for the weekend: dinner at home, a cake and the garden for the kids. Please confirm."),
    ("family@home.example", "Flight and hotel booked", "The flight and the hotel are booked for the holidays. Schedule attached, regards from the whole family."),
    ("family@home.example", "Recipe you asked for", "Here is the recipe you asked for. Dinner was lovely, thanks, and the garden is ready for the weekend."),
    ("colleague@work.example", "Budget review follow-up", "Follow-up on the budget review meeting. Minutes and the updated agenda are attached, deadline is Friday."),
    ("colleague@work.example", "Roadmap feedback", "Feedback on the roadmap: the schedule for the project looks fine, please review the draft notes."),
    ("colleague@work.example", "Lunch with the team", "Lunch with the team on Friday after the presentation. Regards, and thanks for the feedback on the report."),
    ("team@work.example", "Quarterly meeting agenda", "Agenda for the quarterly meeting: project report, budget, roadmap and feedback on the presentation."),
    ("team@work.example", "Deadline moved", "The deadline for the report moved to next week. Project schedule and meeting notes are in the folder."),
    ("team@work.example", "Thanks for the draft", "Thanks for the draft. I added notes for the meeting and fixed the schedule, regards."),
    ("friend@mail.example", "Weekend trip photos", "Photos from the weekend trip are in the album. Dinner at the hotel was great, thanks for the recipe."),
    ("friend@mail.example", "Garden party", "Garden party next weekend, with lunch and a birthday cake. Please bring the photos, regards."),
    ("friend@mail.example", "Flight times", "Flight times for the trip: schedule attached. Hotel is near the garden, dinner on the first night."),
    ("family@home.example", "Holiday schedule", "Holiday schedule for the family: flight, hotel and the birthday dinner. Thanks for the notes."),
    ("family@home.example", "Weekend lunch", "Weekend lunch at home with the family. I will cook the recipe, regards, bring the photos of the garden."),
    ("family@home.example", "Photos of the birthday", "Photos of the birthday are online. Thanks for the dinner, the cake and the garden games."),
    ("colleague@work.example", "Presentation draft", "Draft of the presentation for the meeting tomorrow. Notes on the budget and roadmap are in the last slides."),
    ("colleague@work.example", "Report deadline", "Reminder: the report deadline is Friday. Please review the draft, send feedback and the meeting minutes."),
    ("colleague@work.example", "Team lunch notes", "Notes from the team lunch: schedule for the project, feedback on the roadmap, thanks everyone."),
    ("team@work.example", "Project roadmap review", "Review of the project roadmap on Monday. Agenda, minutes and the budget draft are attached, regards."),
    ("team@work.example", "Meeting tomorrow moved", "The meeting tomorrow moved to the afternoon. Agenda unchanged, please read the report notes."),
    ("team@work.example", "Feedback on the quarterly report", "Feedback on the quarterly report: the budget table is clear, the schedule slide needs a review."),
    ("friend@mail.example", "Dinner recipe", "The recipe for dinner is attached, with photos. Garden vegetables, a nice weekend lunch, regards."),
    ("friend@mail.example", "Birthday weekend", "Birthday weekend at the lake: flight times, hotel and the dinner schedule. Thanks for organising."),
    ("family@home.example", "Garden notes", "Notes for the garden this weekend: lunch outside, photos of the flowers, dinner with the family."),
    ("family@home.example", "Hotel and flight confirmation", "Hotel and flight confirmation for the holidays, with the schedule and the dinner booking. Regards."),
    ("colleague@work.example", "Agenda and minutes", "Agenda and minutes of the project meeting. Feedback on the roadmap and budget is due before the deadline."),
    ("team@work.example", "Presentation feedback", "Feedback on the presentation: good report, clear roadmap, the budget slide needs notes. Thanks, regards."),
];

pub fn spam() -> Vec<Message> {
    SPAM.iter().map(|(from, subject, text)| msg(None, from, subject, text)).collect()
}

pub fn ham() -> Vec<Message> {
    HAM.iter().map(|(from, subject, text)| msg(None, from, subject, text)).collect()
}

/// Held out: shares the junk vocabulary, shares no sender with the corpus.
pub fn spam_probe() -> Message {
    msg(None, "hello@fresh-sender.example", "Winner: free cash prize", "Congratulations winner, click to claim your free cash bonus prize, limited offer, guaranteed.")
}

/// Held out: shares the working-life vocabulary, shares no sender with the corpus.
pub fn ham_probe() -> Message {
    msg(None, "someone@fresh-sender.example", "Notes for the project meeting", "Notes and agenda for the project meeting tomorrow: budget review, roadmap, lunch and the report deadline, regards.")
}

/// Held out: shares no word with either corpus.
pub fn unknown_probe() -> Message {
    msg(None, "nobody@fresh-sender.example", "Zzyzx qwerty", "Quizzical xylophone zephyrs jolt vexed wizards")
}
