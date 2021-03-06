/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use dom::attr::AttrHelpers;
use dom::bindings::codegen::Bindings::AttrBinding::AttrMethods;
use dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use dom::bindings::codegen::InheritTypes::{NodeBase, NodeCast, TextCast};
use dom::bindings::codegen::InheritTypes::{ElementCast, HTMLScriptElementCast};
use dom::bindings::js::{JS, JSRef, Temporary, OptionalRootable, Root};
use dom::bindings::utils::Reflectable;
use dom::document::{Document, DocumentHelpers};
use dom::element::AttributeHandlers;
use dom::htmlelement::HTMLElement;
use dom::htmlheadingelement::{Heading1, Heading2, Heading3, Heading4, Heading5, Heading6};
use dom::htmlformelement::HTMLFormElement;
use dom::htmlscriptelement::HTMLScriptElementHelpers;
use dom::node::NodeHelpers;
use dom::types::*;
use page::Page;

use encoding::all::UTF_8;
use encoding::types::{Encoding, DecodeReplace};

use hubbub::hubbub;
use hubbub::hubbub::{NullNs, HtmlNs, MathMlNs, SvgNs, XLinkNs, XmlNs, XmlNsNs};
use servo_net::resource_task::{Load, LoadData, Payload, Done, ResourceTask, load_whole_resource};
use servo_util::str::DOMString;
use servo_util::task::spawn_named;
use std::ascii::StrAsciiExt;
use std::mem;
use std::cell::RefCell;
use std::comm::{channel, Sender, Receiver};
use url::{Url, UrlParser};
use http::headers::HeaderEnum;
use time;
use string_cache::{Atom, Namespace};

macro_rules! handle_element(
    ($document: expr,
     $localName: expr,
     $string: expr,
     $ctor: ident
     $(, $arg:expr )*) => (
        if $string == $localName.as_slice() {
            return ElementCast::from_temporary($ctor::new($localName, $document $(, $arg)*));
        }
    )
)


pub struct JSFile {
    pub data: String,
    pub url: Option<Url>,
}

pub type JSResult = Vec<JSFile>;

pub enum HTMLInput {
    InputString(String),
    InputUrl(Url),
}

enum JSMessage {
    JSTaskNewFile(Url),
    JSTaskNewInlineScript(String, Option<Url>),
    JSTaskExit
}

/// Messages generated by the HTML parser upon discovery of additional resources
pub enum HtmlDiscoveryMessage {
    HtmlDiscoveredScript(JSResult)
}

pub struct HtmlParserResult {
    pub discovery_port: Receiver<HtmlDiscoveryMessage>,
}

trait NodeWrapping<T> {
    unsafe fn to_hubbub_node(&self) -> hubbub::NodeDataPtr;
}

impl<'a, T: NodeBase+Reflectable> NodeWrapping<T> for JSRef<'a, T> {
    unsafe fn to_hubbub_node(&self) -> hubbub::NodeDataPtr {
        mem::transmute(self.deref())
    }
}

unsafe fn from_hubbub_node<T: Reflectable>(n: hubbub::NodeDataPtr) -> Temporary<T> {
    Temporary::new(JS::from_raw(mem::transmute(n)))
}

fn js_script_listener(to_parent: Sender<HtmlDiscoveryMessage>,
                      from_parent: Receiver<JSMessage>,
                      resource_task: ResourceTask) {
    let mut result_vec = vec!();

    loop {
        match from_parent.recv_opt() {
            Ok(JSTaskNewFile(url)) => {
                match load_whole_resource(&resource_task, url.clone()) {
                    Err(_) => {
                        error!("error loading script {:s}", url.serialize());
                    }
                    Ok((metadata, bytes)) => {
                        let decoded = UTF_8.decode(bytes.as_slice(), DecodeReplace).unwrap();
                        result_vec.push(JSFile {
                            data: decoded.to_string(),
                            url: Some(metadata.final_url),
                        });
                    }
                }
            }
            Ok(JSTaskNewInlineScript(data, url)) => {
                result_vec.push(JSFile { data: data, url: url });
            }
            Ok(JSTaskExit) | Err(()) => {
                break;
            }
        }
    }

    assert!(to_parent.send_opt(HtmlDiscoveredScript(result_vec)).is_ok());
}

// Parses an RFC 2616 compliant date/time string, and returns a localized
// date/time string in a format suitable for document.lastModified.
fn parse_last_modified(timestamp: &str) -> String {
    let format = "%m/%d/%Y %H:%M:%S";

    // RFC 822, updated by RFC 1123
    match time::strptime(timestamp, "%a, %d %b %Y %T %Z") {
        Ok(t) => return t.to_local().strftime(format),
        Err(_) => ()
    }

    // RFC 850, obsoleted by RFC 1036
    match time::strptime(timestamp, "%A, %d-%b-%y %T %Z") {
        Ok(t) => return t.to_local().strftime(format),
        Err(_) => ()
    }

    // ANSI C's asctime() format
    match time::strptime(timestamp, "%c") {
        Ok(t) => t.to_local().strftime(format),
        Err(_) => String::from_str("")
    }
}

// Silly macros to handle constructing      DOM nodes. This produces bad code and should be optimized
// via atomization (issue #85).

pub fn build_element_from_tag(tag: DOMString, ns: Namespace, document: JSRef<Document>) -> Temporary<Element> {
    if ns != ns!(HTML) {
        return Element::new(tag, ns, None, document);
    }

    // TODO (Issue #85): use atoms
    handle_element!(document, tag, "a",         HTMLAnchorElement);
    handle_element!(document, tag, "abbr",      HTMLElement);
    handle_element!(document, tag, "acronym",   HTMLElement);
    handle_element!(document, tag, "address",   HTMLElement);
    handle_element!(document, tag, "applet",    HTMLAppletElement);
    handle_element!(document, tag, "area",      HTMLAreaElement);
    handle_element!(document, tag, "article",   HTMLElement);
    handle_element!(document, tag, "aside",     HTMLElement);
    handle_element!(document, tag, "audio",     HTMLAudioElement);
    handle_element!(document, tag, "b",         HTMLElement);
    handle_element!(document, tag, "base",      HTMLBaseElement);
    handle_element!(document, tag, "bdi",       HTMLElement);
    handle_element!(document, tag, "bdo",       HTMLElement);
    handle_element!(document, tag, "bgsound",   HTMLElement);
    handle_element!(document, tag, "big",       HTMLElement);
    handle_element!(document, tag, "blockquote",HTMLElement);
    handle_element!(document, tag, "body",      HTMLBodyElement);
    handle_element!(document, tag, "br",        HTMLBRElement);
    handle_element!(document, tag, "button",    HTMLButtonElement);
    handle_element!(document, tag, "canvas",    HTMLCanvasElement);
    handle_element!(document, tag, "caption",   HTMLTableCaptionElement);
    handle_element!(document, tag, "center",    HTMLElement);
    handle_element!(document, tag, "cite",      HTMLElement);
    handle_element!(document, tag, "code",      HTMLElement);
    handle_element!(document, tag, "col",       HTMLTableColElement);
    handle_element!(document, tag, "colgroup",  HTMLTableColElement);
    handle_element!(document, tag, "data",      HTMLDataElement);
    handle_element!(document, tag, "datalist",  HTMLDataListElement);
    handle_element!(document, tag, "dd",        HTMLElement);
    handle_element!(document, tag, "del",       HTMLModElement);
    handle_element!(document, tag, "details",   HTMLElement);
    handle_element!(document, tag, "dfn",       HTMLElement);
    handle_element!(document, tag, "dir",       HTMLDirectoryElement);
    handle_element!(document, tag, "div",       HTMLDivElement);
    handle_element!(document, tag, "dl",        HTMLDListElement);
    handle_element!(document, tag, "dt",        HTMLElement);
    handle_element!(document, tag, "em",        HTMLElement);
    handle_element!(document, tag, "embed",     HTMLEmbedElement);
    handle_element!(document, tag, "fieldset",  HTMLFieldSetElement);
    handle_element!(document, tag, "figcaption",HTMLElement);
    handle_element!(document, tag, "figure",    HTMLElement);
    handle_element!(document, tag, "font",      HTMLFontElement);
    handle_element!(document, tag, "footer",    HTMLElement);
    handle_element!(document, tag, "form",      HTMLFormElement);
    handle_element!(document, tag, "frame",     HTMLFrameElement);
    handle_element!(document, tag, "frameset",  HTMLFrameSetElement);
    handle_element!(document, tag, "h1",        HTMLHeadingElement, Heading1);
    handle_element!(document, tag, "h2",        HTMLHeadingElement, Heading2);
    handle_element!(document, tag, "h3",        HTMLHeadingElement, Heading3);
    handle_element!(document, tag, "h4",        HTMLHeadingElement, Heading4);
    handle_element!(document, tag, "h5",        HTMLHeadingElement, Heading5);
    handle_element!(document, tag, "h6",        HTMLHeadingElement, Heading6);
    handle_element!(document, tag, "head",      HTMLHeadElement);
    handle_element!(document, tag, "header",    HTMLElement);
    handle_element!(document, tag, "hgroup",    HTMLElement);
    handle_element!(document, tag, "hr",        HTMLHRElement);
    handle_element!(document, tag, "html",      HTMLHtmlElement);
    handle_element!(document, tag, "i",         HTMLElement);
    handle_element!(document, tag, "iframe",    HTMLIFrameElement);
    handle_element!(document, tag, "img",       HTMLImageElement);
    handle_element!(document, tag, "input",     HTMLInputElement);
    handle_element!(document, tag, "ins",       HTMLModElement);
    handle_element!(document, tag, "isindex",   HTMLElement);
    handle_element!(document, tag, "kbd",       HTMLElement);
    handle_element!(document, tag, "label",     HTMLLabelElement);
    handle_element!(document, tag, "legend",    HTMLLegendElement);
    handle_element!(document, tag, "li",        HTMLLIElement);
    handle_element!(document, tag, "link",      HTMLLinkElement);
    handle_element!(document, tag, "main",      HTMLElement);
    handle_element!(document, tag, "map",       HTMLMapElement);
    handle_element!(document, tag, "mark",      HTMLElement);
    handle_element!(document, tag, "marquee",   HTMLElement);
    handle_element!(document, tag, "meta",      HTMLMetaElement);
    handle_element!(document, tag, "meter",     HTMLMeterElement);
    handle_element!(document, tag, "nav",       HTMLElement);
    handle_element!(document, tag, "nobr",      HTMLElement);
    handle_element!(document, tag, "noframes",  HTMLElement);
    handle_element!(document, tag, "noscript",  HTMLElement);
    handle_element!(document, tag, "object",    HTMLObjectElement);
    handle_element!(document, tag, "ol",        HTMLOListElement);
    handle_element!(document, tag, "optgroup",  HTMLOptGroupElement);
    handle_element!(document, tag, "option",    HTMLOptionElement);
    handle_element!(document, tag, "output",    HTMLOutputElement);
    handle_element!(document, tag, "p",         HTMLParagraphElement);
    handle_element!(document, tag, "param",     HTMLParamElement);
    handle_element!(document, tag, "pre",       HTMLPreElement);
    handle_element!(document, tag, "progress",  HTMLProgressElement);
    handle_element!(document, tag, "q",         HTMLQuoteElement);
    handle_element!(document, tag, "rp",        HTMLElement);
    handle_element!(document, tag, "rt",        HTMLElement);
    handle_element!(document, tag, "ruby",      HTMLElement);
    handle_element!(document, tag, "s",         HTMLElement);
    handle_element!(document, tag, "samp",      HTMLElement);
    handle_element!(document, tag, "script",    HTMLScriptElement);
    handle_element!(document, tag, "section",   HTMLElement);
    handle_element!(document, tag, "select",    HTMLSelectElement);
    handle_element!(document, tag, "small",     HTMLElement);
    handle_element!(document, tag, "source",    HTMLSourceElement);
    handle_element!(document, tag, "spacer",    HTMLElement);
    handle_element!(document, tag, "span",      HTMLSpanElement);
    handle_element!(document, tag, "strike",    HTMLElement);
    handle_element!(document, tag, "strong",    HTMLElement);
    handle_element!(document, tag, "style",     HTMLStyleElement);
    handle_element!(document, tag, "sub",       HTMLElement);
    handle_element!(document, tag, "summary",   HTMLElement);
    handle_element!(document, tag, "sup",       HTMLElement);
    handle_element!(document, tag, "table",     HTMLTableElement);
    handle_element!(document, tag, "tbody",     HTMLTableSectionElement);
    handle_element!(document, tag, "td",        HTMLTableDataCellElement);
    handle_element!(document, tag, "template",  HTMLTemplateElement);
    handle_element!(document, tag, "textarea",  HTMLTextAreaElement);
    handle_element!(document, tag, "th",        HTMLTableHeaderCellElement);
    handle_element!(document, tag, "time",      HTMLTimeElement);
    handle_element!(document, tag, "title",     HTMLTitleElement);
    handle_element!(document, tag, "tr",        HTMLTableRowElement);
    handle_element!(document, tag, "tt",        HTMLElement);
    handle_element!(document, tag, "track",     HTMLTrackElement);
    handle_element!(document, tag, "u",         HTMLElement);
    handle_element!(document, tag, "ul",        HTMLUListElement);
    handle_element!(document, tag, "var",       HTMLElement);
    handle_element!(document, tag, "video",     HTMLVideoElement);
    handle_element!(document, tag, "wbr",       HTMLElement);

    return ElementCast::from_temporary(HTMLUnknownElement::new(tag, document));
}

pub fn parse_html(page: &Page,
                  document: JSRef<Document>,
                  input: HTMLInput,
                  resource_task: ResourceTask)
                  -> HtmlParserResult {
    debug!("Hubbub: parsing {:?}", input);

    // Spawn a JS parser to receive JavaScript.
    let (discovery_chan, discovery_port) = channel();
    let resource_task2 = resource_task.clone();
    let js_result_chan = discovery_chan.clone();
    let (js_chan, js_msg_port) = channel();
    spawn_named("parse_html:js", proc() {
        js_script_listener(js_result_chan, js_msg_port, resource_task2.clone());
    });

    let (base_url, load_response) = match input {
        InputUrl(ref url) => {
            // Wait for the LoadResponse so that the parser knows the final URL.
            let (input_chan, input_port) = channel();
            resource_task.send(Load(LoadData::new(url.clone()), input_chan));
            let load_response = input_port.recv();

            debug!("Fetched page; metadata is {:?}", load_response.metadata);

            load_response.metadata.headers.as_ref().map(|headers| {
                let header = headers.iter().find(|h|
                    h.header_name().as_slice().to_ascii_lower() == "last-modified".to_string()
                );

                match header {
                    Some(h) => document.set_last_modified(
                        parse_last_modified(h.header_value().as_slice())),
                    None => {},
                };
            });

            let base_url = load_response.metadata.final_url.clone();

            {
                // Store the final URL before we start parsing, so that DOM routines
                // (e.g. HTMLImageElement::update_image) can resolve relative URLs
                // correctly.
                *page.mut_url() = Some((base_url.clone(), true));
            }

            (Some(base_url), Some(load_response))
        },
        InputString(_) => {
            match *page.url() {
                Some((ref page_url, _)) => (Some(page_url.clone()), None),
                None => (None, None),
            }
        },
    };

    let mut parser = build_parser(unsafe { document.to_hubbub_node() });
    debug!("created parser");

    let js_chan2 = js_chan.clone();

    let doc_cell = RefCell::new(document);

    let mut tree_handler = hubbub::TreeHandler {
        create_comment: |data: String| {
            debug!("create comment");
            // NOTE: tmp vars are workaround for lifetime issues. Both required.
            let tmp_borrow = doc_cell.borrow();
            let tmp = &*tmp_borrow;
            let comment = Comment::new(data, *tmp).root();
            let comment: JSRef<Node> = NodeCast::from_ref(*comment);
            unsafe { comment.to_hubbub_node() }
        },
        create_doctype: |box hubbub::Doctype { name: name, public_id: public_id, system_id: system_id, ..}: Box<hubbub::Doctype>| {
            debug!("create doctype");
            // NOTE: tmp vars are workaround for lifetime issues. Both required.
            let tmp_borrow = doc_cell.borrow();
            let tmp = &*tmp_borrow;
            let doctype_node = DocumentType::new(name, public_id, system_id, *tmp).root();
            unsafe {
                doctype_node.deref().to_hubbub_node()
            }
        },
        create_element: |tag: Box<hubbub::Tag>| {
            debug!("create element {}", tag.name);
            // NOTE: tmp vars are workaround for lifetime issues. Both required.
            let tmp_borrow = doc_cell.borrow();
            let tmp = &*tmp_borrow;
            let namespace = match tag.ns {
                HtmlNs => ns!(HTML),
                MathMlNs => ns!(MathML),
                SvgNs => ns!(SVG),
                ns => fail!("Not expecting namespace {:?}", ns),
            };
            let element: Root<Element> = build_element_from_tag(tag.name.clone(), namespace, *tmp).root();

            debug!("-- attach attrs");
            for attr in tag.attributes.iter() {
                let (namespace, prefix) = match attr.ns {
                    NullNs => (ns!(""), None),
                    XLinkNs => (ns!(XLink), Some("xlink")),
                    XmlNs => (ns!(XML), Some("xml")),
                    XmlNsNs => (ns!(XMLNS), Some("xmlns")),
                    ns => fail!("Not expecting namespace {:?}", ns),
                };
                element.set_attribute_from_parser(Atom::from_slice(attr.name.as_slice()),
                                                  attr.value.clone(),
                                                  namespace,
                                                  prefix.map(|p| p.to_string()));
            }

            unsafe { element.deref().to_hubbub_node() }
        },
        create_text: |data: String| {
            debug!("create text");
            // NOTE: tmp vars are workaround for lifetime issues. Both required.
            let tmp_borrow = doc_cell.borrow();
            let tmp = &*tmp_borrow;
            let text = Text::new(data, *tmp).root();
            unsafe { text.deref().to_hubbub_node() }
        },
        ref_node: |_| {},
        unref_node: |_| {},
        append_child: |parent: hubbub::NodeDataPtr, child: hubbub::NodeDataPtr| {
            unsafe {
                debug!("append child {:x} {:x}", parent, child);
                let child: Root<Node> = from_hubbub_node(child).root();
                let parent: Root<Node> = from_hubbub_node(parent).root();
                assert!(parent.deref().AppendChild(*child).is_ok());
            }
            child
        },
        insert_before: |_parent, _child| {
            debug!("insert before");
            0u
        },
        remove_child: |_parent, _child| {
            debug!("remove child");
            0u
        },
        clone_node: |_node, deep| {
            debug!("clone node");
            if deep { error!("-- deep clone unimplemented"); }
            fail!("clone node unimplemented")
        },
        reparent_children: |_node, _new_parent| {
            debug!("reparent children");
            0u
        },
        get_parent: |_node, _element_only| {
            debug!("get parent");
            0u
        },
        has_children: |_node| {
            debug!("has children");
            false
        },
        form_associate: |_form, _node| {
            debug!("form associate");
        },
        add_attributes: |_node, _attributes| {
            debug!("add attributes");
        },
        set_quirks_mode: |mode| {
            debug!("set quirks mode");
            // NOTE: tmp vars are workaround for lifetime issues. Both required.
            let tmp_borrow = doc_cell.borrow_mut();
            let tmp = &*tmp_borrow;
            tmp.set_quirks_mode(mode);
        },
        encoding_change: |encname| {
            debug!("encoding change");
            // NOTE: tmp vars are workaround for lifetime issues. Both required.
            let tmp_borrow = doc_cell.borrow_mut();
            let tmp = &*tmp_borrow;
            tmp.set_encoding_name(encname);
        },
        complete_script: |script| {
            unsafe {
                let script = from_hubbub_node::<Node>(script).root();
                let script: Option<JSRef<HTMLScriptElement>> =
                    HTMLScriptElementCast::to_ref(*script);
                let script = match script {
                    Some(script) if script.is_javascript() => script,
                    _ => return,
                };

                let script_element: JSRef<Element> = ElementCast::from_ref(script);
                match script_element.get_attribute(ns!(""), "src").root() {
                    Some(src) => {
                        debug!("found script: {:s}", src.deref().Value());
                        let mut url_parser = UrlParser::new();
                        match base_url {
                            None => (),
                            Some(ref base_url) => {
                                url_parser.base_url(base_url);
                            }
                        };
                        match url_parser.parse(src.deref().value().as_slice()) {
                            Ok(new_url) => js_chan2.send(JSTaskNewFile(new_url)),
                            Err(e) => debug!("Parsing url {:s} failed: {:?}", src.deref().Value(), e)
                        };
                    }
                    None => {
                        let mut data = String::new();
                        let scriptnode: JSRef<Node> = NodeCast::from_ref(script);
                        debug!("iterating over children {:?}", scriptnode.first_child());
                        for child in scriptnode.children() {
                            debug!("child = {:?}", child);
                            let text: JSRef<Text> = TextCast::to_ref(child).unwrap();
                            data.push_str(text.deref().characterdata.data.deref().borrow().as_slice());
                        }

                        debug!("script data = {:?}", data);
                        js_chan2.send(JSTaskNewInlineScript(data, base_url.clone()));
                    }
                }
            }
            debug!("complete script");
        },
        complete_style: |_| {
            // style parsing is handled in element::notify_child_list_changed.
        },
    };
    parser.set_tree_handler(&mut tree_handler);
    debug!("set tree handler");
    debug!("loaded page");
    match input {
        InputString(s) => {
            parser.parse_chunk(s.into_bytes().as_slice());
        },
        InputUrl(url) => {
            let load_response = load_response.unwrap();
            match load_response.metadata.content_type {
                Some((ref t, _)) if t.as_slice().eq_ignore_ascii_case("image") => {
                    let page = format!("<html><body><img src='{:s}' /></body></html>", base_url.as_ref().unwrap().serialize());
                    parser.parse_chunk(page.into_bytes().as_slice());
                },
                _ => loop {
                    match load_response.progress_port.recv() {
                        Payload(data) => {
                            debug!("received data");
                            parser.parse_chunk(data.as_slice());
                        }
                        Done(Err(err)) => {
                            fail!("Failed to load page URL {:s}, error: {:s}", url.serialize(), err);
                        }
                        Done(..) => {
                            break;
                        }
                    }
                }
            }
        },
    }

    debug!("finished parsing");
    js_chan.send(JSTaskExit);

    HtmlParserResult {
        discovery_port: discovery_port,
    }
}

fn build_parser<'a>(node: hubbub::NodeDataPtr) -> hubbub::Parser<'a> {
    let mut parser = hubbub::Parser::new("UTF-8", false);
    parser.set_document_node(node);
    parser.enable_scripting(true);
    parser.enable_styling(true);
    parser
}

