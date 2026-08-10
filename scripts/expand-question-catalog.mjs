import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const catalogPath = resolve(root, "question-packs/games.json");
const games = JSON.parse(readFileSync(catalogPath, "utf8"));
const game = (id) => games.find((item) => item.id === id);
const add = (id, questions) => {
  const target = game(id);
  const seen = new Set(target.questions.map((question) => question.prompt.toLowerCase()));
  for (const question of questions) {
    if (!seen.has(question.prompt.toLowerCase())) {
      target.questions.push(question);
      seen.add(question.prompt.toLowerCase());
    }
  }
};
const choice = (prompt, category, options, visualOptions) => ({ prompt, category, difficulty: "easy", options, ...(visualOptions ? { visualOptions } : {}) });

const thisOrThat = {
  "Food & Drinks": [["Pizza","Burger"],["Coffee","Tea"],["Sweet","Salty"],["Brunch","Midnight snack"],["Homemade meal","Restaurant meal"],["Spicy food","Mild food"],["Chocolate","Vanilla"],["Street food","Fine dining"],["Unlimited desserts","Unlimited snacks"],["Cook together","Order takeaway"]],
  "Funny Situations": [["Laugh at the wrong moment","Trip in front of everyone"],["Always speak in rhyme","Always whisper"],["Sneeze glitter","Hiccup bubbles"],["Wear shoes on the wrong feet","Wear every shirt backwards"],["Accidentally reply all","Accidentally like a five-year-old post"],["Be followed by theme music","Have a laugh track"],["Dance whenever music plays","Sing every text message"],["Lose your keys","Forget why you entered a room"],["Awkward silence","Awkward small talk"],["Tell the joke","Be the joke"]],
  Friendship: [["One best friend","Big friend group"],["Spontaneous plans","Planned hangouts"],["Voice notes","Long calls"],["Honest advice","Comfort first"],["Friends since childhood","Friends who feel brand new"],["Game night","Movie night"],["Roast each other","Hype each other"],["Matching outfits","Matching playlists"],["Share food","Share secrets"],["Road trip together","Staycation together"]],
  "Love & Relationships": [["Love marriage","Arranged marriage"],["Best friend","Romantic partner"],["Grand gesture","Small daily effort"],["Date night out","Cosy date at home"],["Opposites attract","Similar personalities"],["Text all day","One long evening call"],["Surprise gift","Thoughtful letter"],["First love","Last love"],["Shared hobbies","Separate hobbies"],["Plan the future","Live in the moment"]],
  Money: [["Spend money","Save money"],["High salary, less free time","Average salary, more free time"],["Dream house","Travel every year"],["Cash","Card"],["Buy quality once","Buy bargains often"],["Invest early","Enjoy it now"],["Split every bill","Take turns paying"],["Win a million now","Earn ten million slowly"],["Free rent","Free food"],["Rich and unknown","Famous with average income"]],
  Gaming: [["Console","PC"],["Single-player story","Online multiplayer"],["Strategy","Action"],["Retro games","Latest releases"],["Play to win","Play for chaos"],["Open world","Linear adventure"],["Controller","Keyboard and mouse"],["Co-op","Battle royale"],["Great graphics","Great gameplay"],["One favourite forever","A new game every week"]],
  "Movies & TV": [["Netflix","YouTube"],["Marvel","DC"],["Comedy","Thriller"],["Cinema","Home streaming"],["One long series","Many short films"],["Subtitles","Dubbed audio"],["Rewatch a favourite","Try something new"],["Hero story","Villain story"],["Happy ending","Plot twist ending"],["Books","Movies"]],
  Music: [["Headphones","Speakers"],["Live concert","Perfect studio album"],["Old favourites","New releases"],["Lyrics","Beat"],["Solo artist","Band"],["Sing along","Dance along"],["One playlist","Shuffle everything"],["Morning music","Late-night music"],["Front row","Best sound in the middle"],["Discover a hidden artist","Meet your favourite artist"]],
  "Social Media": [["Instagram","TikTok"],["Post everything","Stay private"],["Memes","Reels"],["Never use social media again","Never watch TV again"],["Go viral once","Have a small loyal following"],["Perfect feed","Funny feed"],["Texting","Video calls"],["Read comments","Never read comments"],["Public account","Private account"],["Delete an old post","Keep the memories"]],
  Travel: [["Beach","Mountains"],["Road trip","Flight"],["Luxury hotel","Cosy cabin"],["City break","Nature escape"],["Travel alone","Travel with friends"],["Detailed itinerary","Explore freely"],["Window seat","Aisle seat"],["Return to a favourite","Visit somewhere new"],["Sunrise adventure","Late-night adventure"],["One month abroad","Many weekend trips"]],
  Lifestyle: [["Morning person","Night owl"],["Minimalist home","Cosy clutter"],["Busy social calendar","Quiet weekends"],["Gym workout","Outdoor walk"],["Dress up","Keep it casual"],["Big city","Small town"],["Routine","Variety"],["Early dinner","Late dinner"],["Always on time","Fashionably late"],["Productive Sunday","Lazy Sunday"]],
  "Daily Life": [["Shower in the morning","Shower at night"],["Clean as you go","Clean everything later"],["Alarm clock","Wake naturally"],["To-do list","Remember it all"],["Cook breakfast","Skip to lunch"],["Window open","Room cosy"],["Call","Text"],["Online errands","Go in person"],["One big shop","Several small shops"],["Tidy desk","Creative mess"]],
  "School & University": [["Group project","Solo assignment"],["Morning classes","Evening classes"],["Exam","Presentation"],["Library study","Study at home"],["Take perfect notes","Listen closely"],["Favourite teacher","Favourite subject"],["School trip","Sports day"],["Uniform","Wear your own clothes"],["Learn online","Learn in person"],["Graduate early","Enjoy every semester"]],
  Careers: [["Dream job, average pay","Boring job, amazing pay"],["Work from home","Work in an office"],["Lead the team","Master your craft"],["Stable career","Risky startup"],["Four-day week","Shorter workdays"],["Creative freedom","Clear instructions"],["Travel for work","Never commute"],["Public praise","Private bonus"],["Work with friends","Keep work separate"],["Retire early","Work on something meaningful"]],
  Cars: [["Ferrari","Lamborghini"],["Electric car","Classic petrol car"],["Sports car","Luxury SUV"],["Drive","Be driven"],["Manual","Automatic"],["Fast car","Comfortable car"],["Roadster","Off-roader"],["Perfect sound system","Perfect interior"],["Own one dream car","Try a new car every year"],["Scenic route","Fastest route"]],
  Animals: [["Cats","Dogs"],["Dolphins","Penguins"],["Lion","Tiger"],["Tiny dragon","Giant friendly dog"],["Talk to animals","Understand every bird"],["Safari","Aquarium"],["Pet that talks","Pet that teleports"],["Panda","Koala"],["Horse riding","Swimming with dolphins"],["Rescue pet","Raise a pet from young"]],
  Shopping: [["Online shopping","In-store shopping"],["One luxury item","Five useful items"],["Wishlist","Impulse buy"],["New clothes","New tech"],["Big sale","Perfect item at full price"],["Shop alone","Shop with a friend"],["Brand name","Best quality"],["Free delivery","Same-day pickup"],["Gift card","Mystery gift"],["Keep the receipt","Trust the choice"]],
  Sports: [["Football","Cricket"],["Play","Watch"],["Team sport","Solo sport"],["Indoor","Outdoor"],["Last-minute winner","Dominant victory"],["Speed","Strength"],["Summer Olympics","Winter Olympics"],["Train every day","Big game once a week"],["Home crowd","Away-day adventure"],["Trophy","Personal record"]],
  "Random & Weird": [["Invisible for a day","Read minds for an hour"],["Live without music","Live without films"],["Always know the time","Always know the weather"],["Moon holiday","Underwater hotel"],["Only speak truth","Hear every thought"],["Pause time","Rewind ten minutes"],["Have three arms","Have eyes in the back of your head"],["Never queue","Never hit traffic"],["Swap lives with a friend","Swap lives with a celebrity"],["Live in a treehouse","Live on a houseboat"]],
  Hypothetical: [["Super speed","Teleportation"],["Know your future","Change your past"],["Be lucky","Be brilliant"],["Unlimited time","Unlimited money"],["Perfect memory","Learn anything instantly"],["Save the world anonymously","Be celebrated for a small invention"],["Live 200 years","Live one perfect century"],["Explore space","Explore the deep ocean"],["One wish today","Three wishes in ten years"],["Restart this year","Skip to next year"]],
};

game("this-or-that").questions = game("this-or-that").questions.filter((question) => !/choose your side #/i.test(question.prompt));
const expandedThisOrThat = Object.entries(thisOrThat).flatMap(([category, pairs]) => pairs.map((options) => choice(`${options[0]} or ${options[1]} — which one wins?`, category, options)));
add("this-or-that", expandedThisOrThat);

add("priority-sync", [
  "Who would turn a two-minute story into a full documentary?", "Who would survive longest with only 2% phone battery?", "Who gives the best advice but never follows it?", "Who would accidentally become famous?", "Who is most likely to plan the perfect surprise?", "Who would be the calmest during a travel disaster?", "Who could make friends in any room?", "Who would win a debate using pure confidence?", "Who always knows the best place to eat?", "Who would adopt a pet without warning?", "Who could keep the biggest secret?", "Who would thrive on a reality show?"
].map((prompt,index) => ({prompt,category:["friendship","chaos","lifestyle","future"][index%4],difficulty:"easy"})));

add("would-you-rather", [
  ["Have unlimited travel","Have your dream home"],["Always say what you think","Always know what others think"],["Relive your best day","Erase your worst day"],["Be hilarious","Be incredibly wise"],["Lose your phone for a month","Lose music for a month"],["Host every party","Never organise another plan"],["Have perfect luck","Have perfect timing"],["Meet your future self","Meet your childhood self"],["Be able to pause time","Be able to undo one mistake a day"],["Get free flights","Get free restaurants"],["Live beside all your friends","Travel constantly with them"],["Know every language","Play every instrument"]
].map(([a,b],index) => choice(`Would you rather ${a.toLowerCase()} or ${b.toLowerCase()}?`,["life","friends","future","powers"][index%4],[a,b])));

add("most-likely-to", ["Who is most likely to become a meme?","Who is most likely to miss a flight while already at the airport?","Who is most likely to start a successful side hustle?","Who is most likely to reply after three business days?","Who is most likely to win a karaoke contest?","Who is most likely to move abroad on impulse?","Who is most likely to remember everyone's birthday?","Who is most likely to order food for the whole table?","Who is most likely to befriend a celebrity?","Who is most likely to laugh during a serious moment?","Who is most likely to own the coolest home?","Who is most likely to disappear from the group chat then return with news?"].map((prompt,index)=>({prompt,category:["chaos","friendship","future","party"][index%4],difficulty:"easy"})));

add("never-have-i-ever", ["Never have I ever sent a message to the wrong person.","Never have I ever pretended to understand a film ending.","Never have I ever laughed so hard I cried.","Never have I ever missed a stop because I was on my phone.","Never have I ever eaten someone else's labelled food.","Never have I ever stayed awake until sunrise talking.","Never have I ever made an excuse to leave a video call.","Never have I ever bought something because of a trend.","Never have I ever forgotten a close friend's birthday.","Never have I ever rehearsed an argument in the shower.","Never have I ever searched my own name online.","Never have I ever cancelled plans and felt instantly relaxed."].map((prompt,index)=>choice(prompt,["friends","everyday","funny","social"][index%4],["I have","Not me"])))

add("truth-or-dare", [
  ["Truth: what is your funniest recent search?","Dare: recreate your favourite emoji with your face"],["Truth: which habit would your friends roast first?","Dare: give a dramatic weather report for this room"],["Truth: what harmless lie do you tell most often?","Dare: sell the nearest object like a luxury product"],["Truth: who here would you trust to plan your holiday?","Dare: invent a handshake with the player beside you"],["Truth: what song instantly changes your mood?","Dare: perform ten seconds of an imaginary music video"],["Truth: what tiny thing makes you irrationally happy?","Dare: narrate your next action like a sports commentator"],["Truth: what was your most awkward first impression?","Dare: speak in a movie-trailer voice until your next turn"],["Truth: which app gets too much of your time?","Dare: pose for an imaginary magazine cover"],["Truth: what skill do you wish you had overnight?","Dare: give everyone a playful superhero name"],["Truth: what is your most chaotic travel memory?","Dare: do your best slow-motion victory celebration"],["Truth: what opinion will you defend forever?","Dare: create a three-step dance and teach it"],["Truth: what compliment do you remember most?","Dare: deliver a one-line acceptance speech for winning tonight"]
].map((options,index)=>choice(`Spotlight challenge ${index+1}`,["friends","funny","creative","party"][index%4],options)));

add("word-association", [["Sunrise","Alarm","Coffee","Fresh"],["Weekend","Sleep","Friends","Adventure"],["Group chat","Memes","Plans","Chaos"],["Holiday","Beach","Food","Photos"],["Success","Money","Freedom","Pride"],["Home","Family","Comfort","Wi-Fi"],["Music","Dance","Lyrics","Memories"],["Future","Exciting","Unknown","Bright"],["Rain","Cosy","Traffic","Fresh"],["Friendship","Trust","Laughter","Loyalty"],["Phone","Messages","Camera","Distraction"],["Party","Games","Dancing","Snacks"]].map((options,index)=>choice(`Which word connects first: ${["sunrise","weekend","group chat","holiday","success","home","music","future","rain","friendship","phone","party"][index]}?`,"instinct",options)));

add("guess-the-emoji", [
  ["🧊 🚢 💔","Titanic",["Titanic","Frozen","Jaws","The Notebook"]],["🦁 👑 🌅","The Lion King",["The Lion King","Madagascar","Gladiator","Tarzan"]],["👻 🚫 🚗","Ghostbusters",["Ghostbusters","Cars","Casper","Fast & Furious"]],["🕷️ 👨 🏙️","Spider-Man",["Spider-Man","Batman","Superman","Ant-Man"]],["🐼 🥋","Kung Fu Panda",["Kung Fu Panda","Karate Kid","Mulan","Rocky"]],["👠 🕛 🎃","Cinderella",["Cinderella","Frozen","Barbie","Pretty Woman"]],["🚀 🌕 👨‍🚀","Moon landing",["Moon landing","Star Wars","Rocket League","Gravity"]],["☕ ❤️ 📖","Cosy reading",["Cosy reading","Coffee shop","Study time","Morning meeting"]],["📱 🔋 1️⃣","Low battery",["Low battery","New phone","No signal","Airplane mode"]],["🌧️ 🍿 🎬","Movie night",["Movie night","Rain delay","Cinema trip","Weekend plans"]],["✈️ 🧳 🌍","World travel",["World travel","Lost luggage","Airport job","Map reading"]],["🎤 ⭐ 🏆","Singing champion",["Singing champion","Award show","Pop star","Karaoke night"]]
].map(([prompt,answer,options],index)=>({prompt:`Decode this emoji clue: ${prompt}`,category:["films","phrases","everyday","travel"][index%4],difficulty:"easy",options,correctOption:options.indexOf(answer)})));

add("two-truths-one-lie", [
  ["Octopuses have three hearts.","Bananas are berries.","Goldfish have a three-second memory."],
  ["Venus is hotter than Mercury.","A day on Venus is longer than its year.","Venus has two moons."],
  ["Honey can stay edible for thousands of years.","Apples float because they contain air.","Carrots were invented in 1998."],
  ["Scotland's national animal is the unicorn.","Oxford University is older than the Aztec Empire.","The Eiffel Tower is in Rome."],
  ["A group of flamingos is called a flamboyance.","Sea otters hold hands while sleeping.","Penguins can fly short distances."],
  ["The first webcam watched a coffee pot.","The first computer mouse was made of wood.","Bluetooth is named after a blue whale."],
  ["Wombat droppings are cube-shaped.","Cows have best friends.","Koalas are bears."],
  ["Sound travels faster in water than air.","Lightning is hotter than the Sun's surface.","The Moon makes its own light."],
  ["The shortest war lasted under an hour.","Nintendo was founded in the 1800s.","Chess was invented on Mars."],
  ["Some turtles breathe through their rear end.","Sharks existed before trees.","All sharks must keep swimming every second."],
  ["A cloud can weigh over a million pounds.","There are lakes beneath Antarctica.","Rainbows only have four colours."],
  ["Your body contains enough iron to make a small nail.","Humans share DNA with bananas.","Adults have more bones than babies."],
  ["The hashtag symbol is called an octothorpe.","Email existed before the public web.","Wi-Fi stands for Wireless Fidelity officially."],
  ["A jiffy is a real unit of time.","Hot water can freeze faster than cold water.","Glass is a slow-moving liquid at room temperature."]
].map((options,index)=>({prompt:`Spot the false fact in set ${index+1}.`,category:["nature","space","food","world","animals","technology","science"][index%7],difficulty:index>7?"medium":"easy",options,correctOption:2})));

add("rapid-fire-quiz", [
  ["Which planet is known as the Red Planet?",["Mars","Venus","Jupiter","Mercury"],0],["How many sides does a hexagon have?",["Six","Five","Seven","Eight"],0],["Which ocean is the largest?",["Pacific","Atlantic","Indian","Arctic"],0],["What is the capital of Japan?",["Tokyo","Seoul","Kyoto","Osaka"],0],["Which animal is the fastest on land?",["Cheetah","Lion","Horse","Ostrich"],0],["What does CPU stand for?",["Central Processing Unit","Computer Power Utility","Core Program User","Central Pixel Unit"],0],["Which instrument has 88 keys?",["Piano","Guitar","Violin","Flute"],0],["What is the largest mammal?",["Blue whale","Elephant","Giraffe","Orca"],0],["Which country gifted the Statue of Liberty?",["France","Spain","Italy","Canada"],0],["What is H2O commonly called?",["Water","Oxygen","Hydrogen","Salt"],0]
].map(([prompt,options,correctOption],index)=>({prompt,category:["science","general","world","technology","music"][index%5],difficulty:"easy",options,correctOption})));

add("most-likely-to", ["Who is most likely to turn a quick errand into an adventure?","Who is most likely to know the answer without knowing why?","Who is most likely to organise the reunion ten years from now?"].map((prompt,index)=>({prompt,category:["chaos","personality","future"][index],difficulty:"easy"})));
add("never-have-i-ever", ["Never have I ever waved back at someone who was not waving at me.","Never have I ever opened the fridge and forgotten why.","Never have I ever made a playlist for an imaginary event.","Never have I ever blamed autocorrect for my own typo.","Never have I ever watched an entire series in one weekend."].map((prompt,index)=>choice(prompt,["funny","everyday","music","social","culture"][index],["I have","Not me"])));
add("truth-or-dare", [["Truth: what is the funniest nickname you have had?","Dare: invent a slogan for this friend group"],["Truth: which trend did you secretly enjoy?","Dare: give a five-second motivational speech to a snack"]].map((options,index)=>choice(`Bonus spotlight challenge ${index+1}`,index?"funny":"friends",options)));
add("rapid-fire-quiz", [
  ["Which gas do plants absorb?",["Carbon dioxide","Oxygen","Helium","Hydrogen"]],
  ["How many minutes are in two hours?",["120","100","90","180"]],
  ["Which continent is Brazil in?",["South America","Europe","Asia","Africa"]],
  ["What is the square root of 81?",["9","8","7","6"]],
  ["Which bird is associated with delivering babies in stories?",["Stork","Eagle","Owl","Swan"]]
].map(([prompt,options],index)=>({prompt,category:["science","general","world","general","culture"][index],difficulty:"easy",options,correctOption:0})));

writeFileSync(catalogPath, `${JSON.stringify(games, null, 2)}\n`);
console.log(games.map(({id, questions}) => `${id}: ${questions.length}`).join("\n"));
