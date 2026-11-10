//! macOS : le micro par AVAudioEngine, la transcription par le framework Speech.
//!
//! Une écoute, c'est trois objets : le moteur audio, dont une prise (« tap ») sur l'entrée donne
//! le son du micro ; la requête, qui reçoit ce son ; et la tâche de reconnaissance, qui rappelle
//! avec le texte compris jusque-là. Les rappels n'ont qu'une chose à faire : poster un événement
//! dans un canal, que l'app relève à son rythme.

use std::ptr::NonNull;
use std::sync::mpsc::{Receiver, Sender, channel};

use block2::RcBlock;
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_avf_audio::{
    AVAudioApplication, AVAudioApplicationRecordPermission, AVAudioEngine, AVAudioPCMBuffer, AVAudioTime,
};
use objc2_foundation::{NSBundle, NSError, NSLocale, NSString};
use objc2_speech::{
    SFSpeechAudioBufferRecognitionRequest, SFSpeechRecognitionResult, SFSpeechRecognitionTask,
    SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus,
};

use super::{Access, EarError, EarEvent, EarStatus, Permission};

/// La langue écoutée : Capri parle français.
const LANGUAGE: &str = "fr-FR";
/// Ce que l'`Info.plist` de l'app doit expliquer pour que le système accepte de demander.
const USAGE_KEYS: [&str; 2] = ["NSMicrophoneUsageDescription", "NSSpeechRecognitionUsageDescription"];
/// La prise sur l'entrée du micro : son numéro de bus, et la taille de ses paquets de son.
const INPUT_BUS: usize = 0;
const TAP_FRAMES: u32 = 1024;
/// Les événements qui ne viennent d'aucune écoute en particulier (réponse à une permission).
const NO_SESSION: u64 = 0;

pub struct Ear {
    /// Chaque événement porte le numéro de l'écoute qui l'a produit : ceux d'une écoute
    /// abandonnée, arrivés en retard, ne sont pas pris pour ceux de la suivante.
    events: Receiver<(u64, EarEvent)>,
    sender: Sender<(u64, EarEvent)>,
    recognizer: Option<Retained<SFSpeechRecognizer>>,
    session: Option<Session>,
    sessions: u64,
}

/// Une écoute en cours.
struct Session {
    id: u64,
    engine: Retained<AVAudioEngine>,
    request: Retained<SFSpeechAudioBufferRecognitionRequest>,
    task: Retained<SFSpeechRecognitionTask>,
    /// Le micro est-il encore ouvert ?
    microphone_open: bool,
}

impl Session {
    /// Ferme le micro et dit à la reconnaissance qu'il n'y aura plus de son.
    fn close_microphone(&mut self) {
        if !std::mem::replace(&mut self.microphone_open, false) {
            return;
        }
        log::debug!("écoute {} : micro fermé", self.id);
        // SAFETY: le moteur, sa prise et la requête sont ceux que cette écoute a créés et
        // retient ; ces appels sont faits sur le thread qui les a créés, et retirer la prise
        // posée au démarrage est toujours permis.
        unsafe {
            self.engine.stop();
            self.engine.inputNode().removeTapOnBus(INPUT_BUS);
            self.request.endAudio();
        }
    }
}

impl Ear {
    pub fn new() -> Option<Self> {
        let (sender, events) = channel();
        Some(Self { events, sender, recognizer: None, session: None, sessions: NO_SESSION })
    }

    pub fn status() -> Option<EarStatus> {
        Some(EarStatus {
            in_app: runs_in_its_app(),
            microphone: microphone_access(),
            speech_recognition: speech_access(),
            on_device: new_recognizer().is_some_and(|recognizer| can_transcribe_here(&recognizer)),
        })
    }

    pub fn start(&mut self) -> Result<(), EarError> {
        self.cancel();
        // Sans explication dans l'Info.plist, le système tue l'app qui demande : ne rien demander.
        if !runs_in_its_app() {
            return Err(EarError::NeedsApp);
        }
        let (speech, microphone) = (speech_access(), microphone_access());
        if speech == Access::NotAsked {
            self.ask_for_speech_recognition();
        }
        if microphone == Access::NotAsked {
            self.ask_for_microphone();
        }
        match (speech, microphone) {
            (Access::NotAsked, _) | (_, Access::NotAsked) => return Err(EarError::Asking),
            (Access::Denied, _) => return Err(EarError::Denied(Permission::SpeechRecognition)),
            (_, Access::Denied) => return Err(EarError::Denied(Permission::Microphone)),
            (Access::Granted, Access::Granted) => {}
        }

        if self.recognizer.is_none() {
            self.recognizer = new_recognizer();
        }
        let recognizer =
            self.recognizer.as_ref().filter(|r| can_transcribe_here(r)).ok_or(EarError::Unavailable)?;

        // SAFETY: création d'objets sans argument, puis réglages simples sur la requête que nous
        // venons de créer. `setRequiresOnDeviceRecognition(true)` est la garantie que le son ne
        // part pas chez Apple : sans reconnaissance locale, la tâche échoue au lieu d'envoyer.
        let (engine, request) = unsafe {
            let request = SFSpeechAudioBufferRecognitionRequest::new();
            request.setRequiresOnDeviceRecognition(true);
            request.setShouldReportPartialResults(true);
            request.setAddsPunctuation(true);
            (AVAudioEngine::new(), request)
        };
        // SAFETY: lecture du format de l'entrée du moteur que nous venons de créer.
        let (input, format) = unsafe {
            let input = engine.inputNode();
            let format = input.outputFormatForBus(INPUT_BUS);
            (input, format)
        };
        // SAFETY: simples lectures de propriétés du format. Sans micro, il est vide, et poser
        // une prise dessus lèverait une exception : on s'arrête avant.
        if unsafe { format.sampleRate() <= 0.0 || format.channelCount() == 0 } {
            return Err(EarError::NoMicrophone);
        }

        let tap = {
            let request = request.clone();
            RcBlock::new(move |buffer: NonNull<AVAudioPCMBuffer>, _when: NonNull<AVAudioTime>| {
                // SAFETY: le système passe un paquet de son valide le temps de l'appel. Ajouter
                // du son à la requête depuis le thread audio est l'usage prévu par Apple, et la
                // requête reste vivante : ce bloc la retient.
                unsafe { request.appendAudioPCMBuffer(buffer.as_ref()) };
            })
        };
        // SAFETY: le bus 0 est l'entrée du micro, le format est celui que le nœud annonce, et
        // le moteur est neuf : aucune prise n'y est déjà posée. Le nœud garde sa propre copie
        // du bloc.
        unsafe {
            input.installTapOnBus_bufferSize_format_block(
                INPUT_BUS,
                TAP_FRAMES,
                Some(&format),
                RcBlock::as_ptr(&tap),
            );
            engine.prepare();
        }
        // SAFETY: démarrage du moteur préparé juste au-dessus ; l'erreur éventuelle est rendue.
        if let Err(error) = unsafe { engine.startAndReturnError() } {
            log::warn!("le micro ne démarre pas : {} ({})", error.localizedDescription(), error.code());
            // SAFETY: retire la prise posée juste au-dessus, sur le même nœud.
            unsafe { input.removeTapOnBus(INPUT_BUS) };
            return Err(EarError::NoMicrophone);
        }

        self.sessions += 1;
        let id = self.sessions;
        // SAFETY: simples lectures de propriétés du format, vivant ici.
        let (rate, channels) = unsafe { (format.sampleRate(), format.channelCount()) };
        log::debug!("écoute {id} : micro ouvert ({rate} Hz, {channels} canal ou canaux)");
        let heard = {
            let sender = self.sender.clone();
            RcBlock::new(move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
                // SAFETY: le système passe un résultat vivant le temps de l'appel, ou nil.
                let result = unsafe { result.as_ref() };
                // SAFETY: de même pour l'erreur, vivante le temps de l'appel, ou nil.
                let error = unsafe { error.as_ref() };
                let event = match result {
                    // SAFETY: simples lectures de propriétés du résultat, vivant ici.
                    Some(result) => unsafe {
                        EarEvent::Heard {
                            text: result.bestTranscription().formattedString().to_string(),
                            last: result.isFinal(),
                        }
                    },
                    None => {
                        // « rien d'intelligible » arrive ici aussi : ce n'est pas une panne
                        if let Some(error) = error {
                            log::debug!("écoute terminée sans texte : {} {}", error.domain(), error.code());
                        }
                        EarEvent::Failed
                    }
                };
                // ce qui a été dit n'est jamais écrit dans le journal : seulement posté à l'app
                let _ = sender.send((id, event));
            })
        };
        // SAFETY: la requête est celle créée plus haut, que la tâche retient ; le système garde
        // sa propre copie du bloc et le rappelle sur la file principale, celle de ce thread.
        let task = unsafe { recognizer.recognitionTaskWithRequest_resultHandler(&request, &heard) };
        self.session = Some(Session { id, engine, request, task, microphone_open: true });
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(session) = &mut self.session {
            session.close_microphone();
        }
    }

    pub fn cancel(&mut self) {
        if let Some(mut session) = self.session.take() {
            session.close_microphone();
            // SAFETY: la tâche est celle de cette écoute ; l'annuler deux fois, ou après sa fin,
            // est sans effet.
            unsafe { session.task.cancel() };
        }
    }

    pub fn drain(&mut self) -> Vec<EarEvent> {
        let current = self.session.as_ref().map(|session| session.id);
        let events: Vec<EarEvent> = self
            .events
            .try_iter()
            .filter(|(id, _)| *id == NO_SESSION || Some(*id) == current)
            .map(|(_, event)| event)
            .collect();
        let over =
            events.iter().any(|event| matches!(event, EarEvent::Failed | EarEvent::Heard { last: true, .. }));
        if over && let Some(mut session) = self.session.take() {
            // le système a conclu de lui-même (silence, durée maximale) : le micro se ferme
            session.close_microphone();
        }
        events
    }

    fn ask_for_speech_recognition(&self) {
        let sender = self.sender.clone();
        let answered = RcBlock::new(move |_status: SFSpeechRecognizerAuthorizationStatus| {
            let _ = sender.send((NO_SESSION, EarEvent::PermissionAnswered));
        });
        // SAFETY: l'Info.plist de l'app explique cette demande (vérifié par `runs_in_its_app`),
        // sans quoi le système tuerait le processus ; le système garde sa copie du bloc.
        unsafe { SFSpeechRecognizer::requestAuthorization(&answered) };
    }

    fn ask_for_microphone(&self) {
        let sender = self.sender.clone();
        let answered = RcBlock::new(move |_granted: Bool| {
            let _ = sender.send((NO_SESSION, EarEvent::PermissionAnswered));
        });
        // SAFETY: comme pour la reconnaissance vocale, la demande est expliquée dans l'Info.plist.
        unsafe { AVAudioApplication::requestRecordPermissionWithCompletionHandler(&answered) };
    }
}

impl Drop for Ear {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// L'app en cours dit-elle au système pourquoi elle veut le micro et la reconnaissance vocale ?
/// C'est le cas dans `Capri.app`, pas quand le binaire est lancé nu depuis un terminal.
fn runs_in_its_app() -> bool {
    let bundle = NSBundle::mainBundle();
    USAGE_KEYS.iter().all(|key| bundle.objectForInfoDictionaryKey(&NSString::from_str(key)).is_some())
}

fn speech_access() -> Access {
    // SAFETY: simple lecture d'un état, qui ne demande rien à l'utilisateur.
    match unsafe { SFSpeechRecognizer::authorizationStatus() } {
        SFSpeechRecognizerAuthorizationStatus::Authorized => Access::Granted,
        SFSpeechRecognizerAuthorizationStatus::NotDetermined => Access::NotAsked,
        _ => Access::Denied,
    }
}

fn microphone_access() -> Access {
    // SAFETY: simple lecture d'un état, qui ne demande rien à l'utilisateur.
    match unsafe { AVAudioApplication::sharedInstance().recordPermission() } {
        AVAudioApplicationRecordPermission::Granted => Access::Granted,
        AVAudioApplicationRecordPermission::Undetermined => Access::NotAsked,
        _ => Access::Denied,
    }
}

fn new_recognizer() -> Option<Retained<SFSpeechRecognizer>> {
    let locale = NSLocale::localeWithLocaleIdentifier(&NSString::from_str(LANGUAGE));
    // SAFETY: initialise un objet tout juste alloué avec une locale valide ; rend nil si la
    // langue n'est pas prise en charge.
    unsafe { SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &locale) }
}

/// La reconnaissance est-elle disponible maintenant, et sans sortir de la machine ?
fn can_transcribe_here(recognizer: &SFSpeechRecognizer) -> bool {
    // SAFETY: simples lectures de propriétés d'un objet vivant.
    unsafe { recognizer.isAvailable() && recognizer.supportsOnDeviceRecognition() }
}
